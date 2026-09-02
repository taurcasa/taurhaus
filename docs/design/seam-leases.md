# Seam leases — W-C design brief

Workstream C of [`team-feedback-program-2026-09.md`](team-feedback-program-2026-09.md). Field evidence: the fastbreak retro's F3 ownership tax — the court-zone handover, freeze rules issued as inbox prose, a commit-hash handback fished out of a 99-message inbox, idle windows with no wake when the seam cleared, manifest ownership voids, and the lead's own verdict: *"Ownership needs to be a lease the mesh holds, not a sentence I wrote."* Endorsed across four lanes (fd-dev-1, dev-1, product-check-1, design) and adopted in the lead's change list.

This brief was grounded by three research passes: a mesh-internals survey (storage/locking/wake paths/CLI conventions), a retro incident extraction (nine incidents A–I with per-incident requirements), and a lease-lifecycle failure-mode analysis (nine modes, ranked machinery vs documentation). Their conclusions are folded in below rather than referenced.

## What a lease is

A **declarative coordination signal, not an enforcement mechanism**. Mesh never blocks a write, never validates a seam name against the filesystem, never locks a file. The lease makes ownership *queryable* and release *an event that wakes people*, replacing prose freeze rules and hand-composed handback messages. Pinned per-owner worktrees — the wave's strongest keep — remain the physical isolation layer; the lease governs the seam: shared surfaces, the integration point, the moment of handback.

## The one invariant

> **Automated actors (daemons, task-lifecycle hooks, sweeps) may only move a lease *toward* `handback-ready`. Only a named actor — the holder, an evidenced stealer, or the lead — may move it to `released`. Staleness makes a lease stealable, never silently released.**

Rationale: in a signals-only system, a wrongly-released lease causes an invisible double-write into a seam; a wrongly-held lease causes a visible, arbitrable stall. The design always prefers the second failure.

## States and transitions

Three states: **`held` → `handback-ready` → `released`**.

- **`held`** — one holder, actively editing the seam. Acquire against a held lease **enqueues** the requester as a waiter (acquire-or-enqueue: the command never fails; it reports "held by you" or "waiting behind X").
- **`handback-ready`** — the holder's work is complete and the integration point is published (`ref` = commit hash or equivalent, plus an optional note). This is the NO-RUSH state the retro kept: waiters are woken *with the payload* and **may acquire directly from this state** — acquisition atomically clears the previous holder. Nobody has to compose a release notice, and an auto-handback (see task tie) unblocks the next lane without further human action.
- **`released`** — seam fully clear, nobody holds it. The record persists (registry remembers the seam and its last handback).

Transitions and who may perform them:

| Operation | Actor | Effect |
|---|---|---|
| `acquire` | any authenticated active member | becomes holder if `released`/`handback-ready`/absent; enqueues as waiter if `held` |
| `handback` | holder only | → `handback-ready` with `{ref, note}`; wakes all waiters with the payload |
| `release` | holder only | → `released`; wakes all waiters |
| `steal` | any member **with evidence** (holder `isActive == false` or dead pane), or the lead | becomes holder; journaled with actor, evidence, reason |
| `transfer` | lead, `--admin-reason` required | atomic holder swap (the #86 reroute as a first-class op); wakes old and new holder |
| task-lifecycle tie | automation | linked task reaches `completed`/`deleted`/reassigned → `handback-ready` (never `released`), wake holder + waiters |

**Wake semantics: wake-all, grant-to-nobody.** Release and handback wake every waiter via a normal-priority durable inbox message in the house style — self-contained, carries the payload and the exact re-acquire command, survives compaction, and inherits the member daemon's delivery dedup and restart backlog reconciliation for free. The re-acquire race is serialized by the per-lease flock; dead waiters simply never race. No FIFO grant queue: auto-granting hands a lease to a possibly-dead waiter and recreates the dead-holder problem with a holder who never knew they held it. Fairness machinery is theater at ≤8-member scale; the lead arbitrates the rare genuine contention.

## Record and storage

One file per lease: `teams/{team}/state/leases/{name}.json` (name validated by the existing `validate_name`-class rule, path-safe; **never** validated against the filesystem). Mutations use the established `FlockGuard::acquire_or_create` → read → modify → atomic-rename pattern; the inode-retry loop already covers the replace race. Every transition appends a workflow-journal event (`LeaseAcquired` / `LeaseHandbackReady` / `LeaseReleased` / `LeaseStolen` / `LeaseTransferred`) — the journal owns history and delivery dedup; the lease file stays lean.

Shape (camelCase, flattened `extra` map per house style):

```json
{
  "name": "court-screen",
  "state": "held",
  "holder": "fd-dev-1",
  "scope": "CourtScreen component + court layout CSS",
  "paths": ["client/src/court/"],
  "taskId": "96",
  "carveOuts": ["QA may land deterministic test-only fixes here without asking; hand off after"],
  "acquiredAt": "…", "stateChangedAt": "…",
  "handback": { "ref": "2d0d7ca", "note": "integration point for #96", "at": "…" },
  "waiters": [{ "name": "dev-1", "enqueuedAt": "…" }]
}
```

- **Keyed to the member name, never session/pid/pane** — the one identity that survives restart, compaction, and resume. A member restart is a non-event.
- `paths` is advisory display ("hot files"), never enforcement.
- `carveOuts` encodes pre-authorized delta classes as free text (the standing-authorization keep: the boundary is ownership/risk, not file count — a tiny deterministic correction inside the scope shouldn't cost a permission round-trip).
- `taskId` (via `--task N` on acquire, default-encouraged, not mandatory) arms the task-lifecycle tie — the single highest-leverage mechanism: it absorbs holder-compaction amnesia, lease-outlives-task, and most stale-lease-after-resume cases in one move.

## Liveness, arbitration, resume

- **Holder/waiter liveness is displayed, never acted on automatically**: `lease list`/`status` annotate each name with the existing liveness stack (`isActive`, pane probe). Dead-looking holder ⇒ the lease shows as stealable; the evidenced steal replaces arbitration for the common case.
- **Disputes** (a steal without evidence, contested ownership) escalate to the lead's inbox exactly like idle escalations today: durable, reconciled when the lead returns, visible in the guardrail summary so taurhaus can surface it to the human above the lead. No auto-arbitration, no arbitration timeouts.
- **Team resume**: no epoch stamps, no sweep-on-resume. Name-keyed leases survive intact; task-tied leases follow their task's fate; holders not re-onboarded read as inactive and stealable. The one addition: resume onboarding and the compaction card list the member's held leases and waiting positions (see taurhaus companion).
- **Guardrail summary** (team-daemon cycle, the `expired_scaffolds` slot's shape): orphaned leases (held, untied, holder inactive), aged held leases past a display threshold, pending disputes, long-waiting members.

## Idle-monitor interplay

A member with a waiter entry on a held lease is **legitimately non-idle**: the idle monitor suppresses task nudges for them (composes with W-A's nudge-reads-task-state). The wave's cruel inversion — 866 useless wakes while the one valuable wake never fired — must not be recreated by leases turning every frozen window into a nudge target. A long wait surfaces in the guardrail summary instead.

## CLI surface

`mesh lease` follows the `Task { action }` noun-subcommand convention, the standard per-command skeleton (validate → resolve paths → `require_authenticated_active_member` → mutate under lock → journal → best-effort delivery → one-line `[mesh]` stdout, `--json` on reads → implicit activity heartbeat), and lands with USAGE.md lines (clap-parse regression test covers them):

```
mesh lease acquire <name> [--task N] [--scope TEXT] [--path P]... [--carveout TEXT]...
mesh lease handback <name> [--ref COMMIT] [--note TEXT]
mesh lease release <name> [--note TEXT]
mesh lease steal <name> --reason TEXT
mesh lease transfer <name> --to MEMBER --admin-reason TEXT
mesh lease list [--json]
mesh lease status <name> [--json]
```

## Taurhaus companion (small, rides the bundle bump)

1. Compaction reinjection card: one field listing the member's held leases and waiting positions (compose already reads durable snapshots; this is a rendering edit).
2. Resume/reonboard onboarding text: the same line.
3. Optional later: lease display in the mesh canvas / node detail — explicitly out of v1.

## Deliberately not building

- **Filesystem enforcement** of any kind — no locks, no write blocking, no resource-existence validation.
- **TTL auto-expiry** — a long task legitimately holds a seam for hours; auto-release is the invisible-double-write failure. Staleness ⇒ stealable.
- **Heartbeat-renewed leases** — compaction guarantees missed renewals from healthy members; every compaction would become a false expiry.
- **FIFO grant queues / fairness bookkeeping** — wake-all + first-acquire-wins, lead arbitrates contention.
- **Epoch/generation stamps on resume** — composition of name-keying + task tie + steal covers it.
- **Auto-arbitration** — the human is the backstop, as with idle escalations today.
- **Blocking `mesh lease acquire --wait`** — agents are woken through their inbox; a blocking wait is a script convenience deferred until something needs it.
- **Lease-specific reminder nudges** — the task-level idle machinery already owns "holder stalled".

## What leases do not fix (routed elsewhere)

The task-ID swap, compaction restoring stale tasks, mid-flight ruling crossings, and score relay drift are assignment-identity and state-container problems — W-B's task ledger. Stale nudges on *completed* tasks are W-A. The lead outage itself is durability/visibility work riding W-B. A lease serializes *edits to a surface*, not *rulings about it*.

## Tests

- Acquire-or-enqueue under concurrency (two simultaneous acquires: one holder, one waiter — the `tasks.rs` concurrency-test shape).
- Handback and release wake every waiter with payload text; delivery dedup via journal events; a waiter added after handback still sees the payload in `status`.
- Acquire from `handback-ready` atomically clears the previous holder.
- Task-lifecycle tie: linked task completed/reassigned → lease moves to `handback-ready` (never `released`) and wakes.
- Steal is refused without evidence for a non-lead; accepted with holder `isActive == false`; always journaled with reason.
- Transfer is lead-only, requires `--admin-reason`, wakes both parties.
- Idle monitor suppresses nudges for a waiting member.
- Guardrail summary lines for orphaned/aged leases.
- USAGE.md clap-parse regression covers every new invocation.

## Sizing and order

A mesh `feature-pr` lane (CLI + store + daemon touchpoints + idle interplay), plus the two-line taurhaus companion as a `small-change`. **Builds after W-A merges** — both touch `inbox.rs`, the journal, and the idle monitor, and W-C's nudge suppression composes with W-A's task-state gate. Ships in the mesh bundle bump that follows W-A's 0.2.26.

An honest caveat carried from the retro: the seam-lease row was named unprompted by one lane, then adopted by the lead's own verdict and credited to four lanes whose incidents are the same tax seen from different seats. The design above is deliberately minimal — four small mechanisms (task tie; locked acquire-or-enqueue + wake-all; evidenced steal; durable surfacing) over primitives mesh already has, so that if field use disappoints, little was spent.
