# Task ledger — W-B design brief

Workstream B of [`team-feedback-program-2026-09.md`](team-feedback-program-2026-09.md), the architecture piece: **the task record becomes the authoritative, versioned state container.** It unifies four field proposals — research-1's owner epochs, design's ruling ledger, qa-1's resumable checkpoint, fd-dev-1's completion fan-out — into one design over the storage and delivery machinery mesh already has.

Field evidence (F2/F5/F6): reviews ran against superseded packet versions; ruling W5 flipped four times across crossing messages, each flip costing a reconciliation commit; the #31/#32 bare-ID swap started a lane on the wrong task; compaction restored #82 over #99 and #16 over #64; a 36/40 score arrived as 37 twice ("a number in a field cannot be paraphrased"); a lane restart cost 10–15 minutes reconstructing which gates had run; #98's evidence frames took three messages to locate; identical test matrices were hand-copied to four inboxes.

Grounded by three research passes (task-system survey, F2/F5/F6 incident extraction, design-space analysis) whose conclusions are folded in. The governing insight: **most of the proposal already exists.** `assignment_id` + `AssignmentSuperseded` is the epoch; `ASSIGNMENT_CLEAR_KEYS` already invalidates member progress on reassignment; the journal already holds payload history; the per-member daemon loop is already a durable, deduplicated, self-healing notice transport; W-A's `(task_id, kind)` supersession already collapses notice storms. W-B is mostly *verbs checking state the system already maintains*, plus two small new surfaces (rulings, cursor).

## 0. The foundation: mutate-under-lock (fixes a live defect)

Every lifecycle handler today does `get_task` (shared lock) → compute a `TaskUpdate` carrying a **whole replacement metadata object** → `update_task_with_journal`, which re-reads under the dir lock but then overwrites the fresh record's metadata with the stale-snapshot-derived object. Two concurrent metadata writers lose one write — a real lost-update bug independent of W-B.

First deliverable, with a regression test: a closure primitive

```rust
mutate_task_with_journal(ctx, id, actor, f: FnOnce(&Task) -> Result<TaskUpdate>)
```

where `f` sees the truly-current record **inside** the dir lock. The closure can *reject* (the epoch check in §1) and can *sequence* (the ruling `seq` in §2). Every lifecycle verb migrates onto it. This is the one piece of machinery the whole workstream rests on.

## 1. Versioned assignments — echo and rejection, not a new identity

No second identity scheme: **the token is the existing `assignment_id`** (accepted as a ≥8-char prefix). What changes is that verbs finally check it:

- `mesh task accept` and `mesh task start` **require `--assignment <token>`**, verified inside the mutate closure against `current_assignment_id`. A mismatch is a typed error that names both ids and prints the *current* contract (first step / deliverable / completion signal) — the rejection itself performs the re-read the member skipped. The forcing function is the point: the only sources of the token are the assignment notice and `mesh task get`, so a compacted member acting from restored memory either echoes a stale token (bounced, re-synced by the error text) or reads the record. "Restore reads the record, never a message" becomes structural.
- `progress` / `review` / `complete` add **no mandatory token** (ceremony tax, F4). They get a stage-presence guard instead, using state reassignment already clears: reject when `started_at` is absent or `started_by != actor` — which is exactly the fingerprint of acting under a superseded assignment. `--assignment` stays available as an opt-in hard check.
- `complete` on a task with `completed_at` already set is rejected; the lead repairs with the existing `--as-lead --admin-reason` idiom.
- Every generated notice already carries task id + subject + owner (W-A); the assignment notice adds the token.

The versioned payload needs no history array on the record: the current payload lives in metadata (as today, replaced per assign), and the journal's `TaskAssigned` events are the history. Spec pinning is doctrine, not schema: per the W-D delivery standard the committed packet is the spec, so an assignment's deliverable names the packet commit where one exists, and a reviewer's first ruling entry records the hash it opened (its `ref` field).

## 2. Rulings, verdicts, scores — sequenced entries on the record

- `metadata.rulings`: append-only array of `{seq, kind: verdict|score|ruling|note, field?, value, by, at, ref?}`. `seq` is assigned under the dir lock (max + 1) — the dir-wide flock gives total order for free; no vector clocks, no per-entry files, at 7-member/101-task scale contention is irrelevant.
- A ruling that *is* the current answer also sets a scalar (`metadata.review_score`, `metadata.verdict`) in the same locked write, `seq` as tiebreaker. Anti-drift is then a **reader property**: `task get` prints the scalar and the rulings tail, the completion packet quotes the field, and a relay that quotes the record cannot drift 36→37.
- Each entry mirrors as a `RulingRecorded` workflow event. Record-array order (= seq = lock order) is authoritative; journal order stays best-effort audit — an asymmetry every lifecycle event already has, now documented rather than fixed.
- New verb: `mesh task ruling <id> --kind score --value 36 [--field review_score] [--ref <hash>] [--note …]`. Authorization: **any authenticated active member may append** (rulings come from reviewers, who are neither owner nor lead); entries are attributed and sequenced, and status-changing verbs keep their owner/lead gates. The retro's pain was drift, not vandalism.
- Rulings deliberately survive reassignment (not added to `ASSIGNMENT_CLEAR_KEYS`): they judge the work and its artifacts, not the assignment. Entries can carry the assignment id in `ref` for scoping.

## 3. Artifacts and the operational cursor

**Artifacts** — record-resident, low-frequency: `metadata.artifacts: [{path, kind, note, added_by, added_at}]`, appended via the mutate primitive (an `--artifact` flag on `progress`/`review`/`complete`, plus `mesh task artifact add`). Rendered by `task get`, quoted in the completion packet. "Where do the frames live" becomes a lookup.

**Cursor** — the one piece that must NOT go through the canonical path. Every journaled write moves the journal offset, and a moved offset forces the next reader to rebuild projections from the full journal (which never rotates). A cursor updated every few minutes per member would dominate the system's write volume. Therefore an **unjournaled sidecar**, precedented by the runtime/activity records: `state/cursors/{task_id}.json` holding `{last_command, last_result, next_action, updated_by, updated_at}`, written by the owner via plain `atomic_write` (a dying session leaves the previous complete cursor — exactly the desired semantics, from a primitive that exists). Per-task, so it survives an owner change; removed best-effort on complete.

- Write: `mesh task cursor <id> [--command …] [--result …] [--next …]`; cadence is doctrine — step boundaries, not keystrokes.
- Read: `task get` and the recovery bundle's `resume_hint` render it; the taurhaus compaction card points members at `task get`.
- `last_progress` (journaled, lead-notifying milestone) and the cursor (silent restart aid) stay distinct — merging would drag the cursor back through the journal.

## 4. One completion packet

The canonical half already ships (record update + `TaskCompleted` event + lead notice). W-B completes it:

- **The packet is a rendering of the record post-update**: summary, verdict/score scalars, rulings tail, artifacts, assignment id. `mesh task complete --summary … [--artifact …]` emits everything — members stop composing result essays because the verb does the sending, and evidence is "sent" by being on the record and quoted once.
- **Derived subscribers, no registry**: the lead (as today), `assigned_by` if distinct, and the owners of tasks listed in `blocks` — the dependents this completion unblocks, the highest-value new recipient, computed from the record. Each notice carries `(task_id, kind: "completion")`, so W-A's supersession replaces a stale unread completion in place when a correction follows.
- **Commit-first**: the record write is never hostage to delivery; failed fan-out warns loudly (the assign path's existing idiom). The journal-derived "completion notice owed" projection (mirroring the pending-assignment loop, healing the crash window between record commit and inbox appends) is *designed but deferred* — the daemons' backlog reconciliation already heals pane-delivery loss, which is the observed case; build the owed-notice pass only if the crash window is ever seen in the field.
- Generated sends to an inactive recipient print the 0.2.25 broadcast-style note, so a lane writing toward a dead lead knows it (the visible half of the outage finding; durable parking is the inbox itself).

## Compatibility

All W-B state lives **under `metadata`**, where the merge helpers preserve unknown keys against older binaries; additionally `Task` gains `#[serde(flatten)] extra` (matching `Member`/`TeamConfig`) as cheap insurance for the future — unknown *top-level* keys are currently dropped by any older binary's rewrite. Old binaries don't run the new checks; the residual stale-`~/.local/bin/mesh` risk is the same class as the known 0-byte-mesh failure and is an ops note, not a mechanism.

## Deliberately not building

- **A second assignment-identity scheme** (integer epochs beside `assignment_id`) — display sugar at best; one identity system.
- **Payload content fingerprints** on the token — pins identity, not bytes; the dir lock makes the stale-content race narrow. Documented.
- **A parallel checkpoint store** — folded into the cursor sidecar, per the program doc.
- **A subscriber registry** — derived recipients cover the field cases; registration state would rot.
- **CRDTs/vector clocks for rulings** — the dir lock totally orders writes at this scale.
- **Per-task locks** — the dir-wide lock is adequate; `paths::task_lock` stays unused.
- **Journal rotation** — a real, pre-existing concern, deliberately out of scope; W-B keeps high-frequency state (cursor) out of the journals precisely so it stays deferrable.

## What the ledger does not fix

Genuine decision churn (the substance of W5's four reversals — the ledger makes each flip cheap, it doesn't prevent reversals); rework from legitimate late spec changes; text not yet emitted when a session dies; content errors at the source (the 36 was right in the field — a wrong number in a field is caught by review, not schema). The lead-rejoin inbox wipe remains the separate investigation with W-A's tripwire as the trap.

## Tests

- Regression test for the lost-update defect (two racing metadata writers, both writes survive) — lands with §0.
- Accept/start reject a stale token atomically; the error prints the current contract. Accept/start with the current token succeed.
- Progress/complete rejected when `started_at` absent or `started_by != actor`; `complete` on completed rejected; lead `--as-lead` repair path still works and audits.
- Two racing rulings both survive with distinct `seq`; the scalar reflects the later; `RulingRecorded` events carry `seq`.
- Rulings survive reassignment; `ASSIGNMENT_CLEAR_KEYS` behavior unchanged for lifecycle keys.
- Cursor: atomic replacement, stale `updated_at` visible, rendered in `task get`/recovery, removed on complete; never appears in any journal.
- Completion fan-out reaches lead + assigner + blocked-task owners, carries `(task_id, "completion")`, supersedes a prior unread completion, warns-not-fails on a broken recipient inbox, notes inactive recipients.
- USAGE.md clap-parse regression covers every new invocation.

## Sizing and order

A mesh `feature-pr` lane, **after W-C** (program order: W-C is small and independent; W-B is the architecture piece) — and strictly after W-A (shipped, 0.2.26), whose supersession shapes §4 depends on. Machinery priority within the lane: §0 mutate primitive → §1 token + stage guards → §2 rulings → §3 cursor/artifacts → §4 packet. Each stage is independently shippable; §0 alone fixes a live bug. The taurhaus companion (compaction card pointing at `task get`, board rendering of scalars/rulings) rides the bundle bump as a `small-change`.
