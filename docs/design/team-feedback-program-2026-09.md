# Team feedback program — September 2026

Source: the fastbreak wave retro (`fastbreak_rerun/docs/TEAM-RETRO-2026-09-02.md`), the first full-scale field test of the role catalog and coordination stack: 17 hours, 7 members, 101 tasks, a shipped game, a passed human gate — and a precise record of where coordination cost more than it returned.

Operating rule, set by the operator: **the customers (the agents) are right about their findings, not necessarily about their solutions.** This program re-derives solutions from the validated pain. Where a workstream diverges from a lane's proposal, the divergence is stated.

## Findings → root causes

| # | Validated finding (evidence) | Root cause |
|---|---|---|
| F1 | Notification noise: 866 wake injections; idle nudges on completed tasks all session; inbox storms of repeated/duplicate notices burying the real instruction | The notification layer is fire-and-forget: the nudge daemon never consults task state; senders append with no supersession or dedup |
| F2 | State crossings: reviewed v2 while v2.2 existed; W5 flipped 4×; the #31/#32 bare-ID swap; compaction restored stale tasks twice; a score drifted 36→37 in relay | Authoritative state lives in prose messages; order and version are not machine-carried; the task record is not the container of truth |
| F3 | Ownership tax: seams issued as inbox prose; frozen windows with no wake-on-release; hand-run handback coordination | Ownership is a convention, not a state the mesh holds |
| F4 | Ceremony not scaled to work: mandatory deadlines (median task used 16% of its deadline) and effort labels (high ≡ medium at 12 min) predicted nothing; 1.2k-char assignments restating the packet; RESULT essays hiding the worst reviewer error; red-first on scaffolding; screenshot rules on measurement tasks; double review converging ~90% off hero surfaces | One uniform task/role template regardless of work kind and risk |
| F5 | Session-state fragility: a restart lost the operational cursor (10–15 min); the lead's credit-death lost a lane's reply; the lead's inbox was reset to `[]` on rejoin; compaction restored obsolete context | State that lives only in a session's context is state the mesh loses; one confirmed, unattributed inbox truncation |
| F6 | Evidence homelessness: three messages to locate one artifact; identical evidence re-sent to several inboxes | Results have no structured home on the task |

## Workstreams

### W-A — Notification hygiene (mesh 0.2.26; ships first)
- The idle/nudge daemon reads canonical task state before firing: never on a completed/reviewing task. *(F1; the single highest-consensus defect)*
- Every notice carries task id + subject + owner. *(F2's bare-ID class)*
- Delivery-time supersession: an unread notice for the same (task, kind) is replaced in place, not appended. Corrections supersede; storms collapse structurally. *(F1, and the cheap half of F2)*
- Inbox-truncation tripwire: a write that would shrink an inbox holding unread messages quarantines the prior content and warns — the trap for F5's unattributed wipe, and a permanent safety net.
- Mesh accepts tasks without deadline/effort; `--why` never required. *(F4, mesh half)*

*Divergence from proposals:* dev-1's "automatic heartbeat while owned commands run" is deferred — task-state awareness removes the misfires that were actually named; process-awareness is harness-specific machinery we add only if nudges still misfire afterward.

### W-D — Role catalog v2 (taurhaus templates; ships with W-A)
One concept instead of six edits: **work-kind ceremony classes** (measure / diagnose / implement / review / spec-delta) with per-class defaults — evidence rules, red-first requirement (behavior/regression only), review depth (double review on declared hero surfaces only), and artifact shape (reviewer output = numbered findings + score table; prose optional).
- Assignment standard: five lines (objective, deliverable, first action, completion signal, review route); the committed packet is the spec; recurring doctrine lives in one linked delivery-standard doc and is never restated per task.
- Ownership language: one accountable implementer + one acceptance owner per surface, with decision rights; design gets a standing spec-delta lane; QA owns tests/harness/probes with commit authority there; researcher is phase-scoped with named standby triggers.
- Deadlines and effort become optional overrides in doctrine (the mechanisms — the daemon deadline pass, effort switching — remain for the tasks that need them; what retires is the obligation).

### W-B — Task ledger (mesh; design brief then build)
The architecture piece, unifying four proposals (epochs, ruling ledger, checkpoint, completion fan-out) into one: **the task record becomes the authoritative, versioned state container.**
- Versioned assignment payload with owner epoch; accept/start reject a stale epoch atomically; compaction and restore read the record, never a message. *(F2, F5-compaction)*
- Rulings, verdicts, and scores land as sequenced structured entries on the task; a number in a field cannot drift in relay. *(F2)*
- An artifacts list and a lightweight operational cursor (last command/result, next action) live on the record; restore shows them. *(F5, F6 — qa-1's "checkpoint store" folded here rather than built as a parallel store)*
- One completion packet: a result is a record update that fans out to task history and subscribers; evidence is never re-sent by hand. *(F1, F6)*

### W-C — Seam leases (mesh; brief then build, independent of W-B)
Named leases as first-class mesh state: `held` / `handback-ready` / `released`, a holder, waiters, and an automatic wake on release. Declarative coordination — not filesystem enforcement; pinned per-owner worktrees (which earned "keep") remain the physical isolation layer. *(F3)*

### Investigation — the inbox wipe (F5)
`mesh join` is exonerated (O_EXCL, never truncates); no taurhaus writer produces an empty inbox; the displaced sibling was empty. W-A's truncation tripwire is the trap; prime remaining suspect is the Claude Code native teams layer initializing the same file. Lead-outage *visibility* (presence surfaced to senders) rides W-B's delivery work.

## Order

Operator direction: no schedule pressure from a next wave — the goal is building taurhaus into something genuinely useful, so each piece gets built properly.

1. **W-A + W-D in parallel** — defect fixes and policy corrections on existing code; they also de-risk the architecture work (W-A's supersession semantics inform W-B's record schema).
2. **W-C and W-B briefs written with full care, then built** — W-C first (small, independent), then W-B (the architecture piece), each through the full lane machinery.

## What is deliberately not being built

- No process-heartbeat integration (see W-A divergence).
- No parallel checkpoint store (folded into W-B's cursor).
- No enforced file locking (W-C is declarative; worktrees stay the isolation).
- No removal of the deadline/effort *mechanisms* — only their mandatory-ness.
