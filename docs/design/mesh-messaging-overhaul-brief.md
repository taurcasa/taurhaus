# Research brief: mesh messaging architecture overhaul

Commissioned by the operator, 2026-09-07. One Astra researcher at high
effort. Deliverable: `docs/design/mesh-messaging-overhaul-research.md` —
an architecture study ending in a recommended design and a staged
migration sketch. Research only; no implementation.

## The operator's framing

Mesh's messaging was the right idea at the time, but it may profit
greatly from a major overhaul. The seed sketch, offered as a starting
point — improve on it freely:

- **Storage**: one JSONL per task conversation, with clear fields —
  author, creation time, and whatever else earns its place. DMs as one
  JSONL per pair; groups as one per group.
- **Delivery**: a service per team member (or one per team, filtered per
  member) that watches for new messages addressed to that member and
  pushes them.
- **Transport**: possibly retire tmux send-keys for message delivery —
  writers only append to files; the per-member service is the ONLY thing
  that touches tmux, and where a harness can consume input natively, the
  model just monitors the service's output instead of receiving
  keystrokes at all.

If a better architecture exists, propose it — the operator is explicitly
open to being out-designed here.

## Prior work that binds

`docs/design/mesh-task-threads-research.md` (and its brief) just
concluded on the READ side: task history as a bounded projection over
existing records, no default subscriptions, DMs retained, read
economics measured. This study is the WRITE/TRANSPORT side. The two
must compose: whatever storage you propose should make that projection
trivial (the memo found only 12/106 envelopes carried structured task
links, and 58 of 86 sent messages had no retained body — retention and
linking are storage-layer failures).

## What to ground in

- Current mesh storage and delivery: `~/projects/mesh` (READ-ONLY) —
  inbox JSON arrays with locked rewrites (FlockGuard), the workflow
  journal / protocol index / task mutation journal with byte-offset
  readers, generated-notice supersession, broadcast fan-out with
  per-recipient message IDs, the round-4/5 lockless-read and torn-read
  work. What of this is already the right substrate?
- Delivery today: tmux send-keys into panes, plus per-harness native
  paths taurhaus already encodes — `src-tauri/src/session_scanner/cli_tool.rs`
  capability slices, `coordination/compact_hook.rs` (HookStdout vs
  MeshInbox delivery), codex notify hook, agy hooks, grok's env-injected
  hooks. `docs/architecture/harness-model.md` is the authority on what
  taurhaus owns vs what CLIs own.
- **The per-harness inbound-channel question is the crux**: for each of
  claude / codex / agy / grok, what channels exist for getting a message
  INTO a running interactive session without keystrokes (hooks at turn
  boundaries, MCP servers, file-watch conventions, stdin, anything
  documented), what latency/turn-boundary semantics each has, and what
  genuinely requires send-keys today. Be precise about what is verified
  from docs/source versus assumed. Send-keys' known field failures this
  season: interleaving with a member's own typing, pane identity drift,
  the $TMUX inheritance incident, this week's pane-split failures.
- Field evidence: the threads memo's retention findings; wave-1 archive
  facts as already measured (do not redo that census); the delivery
  standard's message conventions; the new machinery that must survive
  (awaiting-GO markers, launch-health records, source-tagged nudges,
  canonical lead inbox, completion fan-out).
- taurhaus consumers: the daemon's scanner/telemetry and the planned
  Scope view read mesh state; `docs/architecture/data-architecture.md`
  for ownership boundaries (the Windows app never writes team state;
  daemon and WSL-native hooks are the writers).

## Design dimensions the study must cover

- **Storage schema**: the JSONL-per-conversation model vs alternatives
  (single event log with per-conversation views; per-member logs with
  task links). Concurrency and durability: append atomicity, partial
  lines, rotation/compaction of long-lived files, cursors as byte
  offsets, crash recovery — compared honestly against today's locked
  array rewrites and their measured failure modes.
- **Identity and authorization**: authorship fields vs today's actor
  auth; can a member forge another's append in a shared filesystem, and
  does that change anything relative to today's model?
- **The delivery service**: process model (per member vs per team),
  relationship to the existing team daemon / idle monitor, what happens
  when it dies, ordering and at-least-once vs exactly-once semantics,
  read receipts vs today's read flags.
- **Send-keys retirement, honestly scoped**: which delivery paths can
  move to native channels per harness TODAY, which need send-keys kept
  as one backend behind the service, and what launching/nudging (out of
  scope for messaging) still needs regardless.
- **What must not break**: lifecycle authority (tasks, rulings,
  assignment generations), the wait/launch-health machinery, taurhaus's
  writer boundary and protocol pairing, compaction reinjection, the
  threads projection, telemetry ingestion. Protocol/schema versioning:
  mesh 0.2.29 is locked and a wave is LIVE — the migration sketch must
  be staged (parallel-write? adapter reads?) with a first shippable
  stage that risks nothing.
- **Cost**: does any of this change the token economics the threads
  memo measured, or is it purely a reliability/latency play? Say which,
  honestly.

## Deliverable shape

The recommended architecture with schema sketches (fields, not code); a
per-harness inbound-channel feasibility matrix with verification status
per claim; the delivery-service design; a staged migration plan with a
smallest-first stage; explicit risks and what the design deliberately
does NOT change; where it improves on the operator's seed sketch and
where the seed sketch was already right; open questions; confidence
stated honestly.

## Constraints

- Read-only everywhere: never write to ~/.claude/teams,
  ~/.claude-account2/teams, ~/projects/mesh, ~/projects/job-hunt, or any
  live team state; never touch the live daemon (port 17233) or the
  operator's tmux server. A NINE-SEAT WAVE IS LIVE — observe nothing,
  probe nothing live. Scratch probing only via the locked mesh binary in
  tempdirs (`--claude-dir <tmp>`), scratch tmux servers (`tmux -L`) if
  needed.
- No credentials anywhere. Quote the wave-1 archive sparingly if at all;
  the threads memo already carries the measurements.
