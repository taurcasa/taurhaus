# Mesh-core team — shelf draft

Drafted 2026-09-07 on the operator's direction. **Status: shelf-ready,
not launched.** Launch is gated on, in order: (1) the communication-
overhead reduction phase below, (2) the wave-2 retro (its roster grades
feed this composition), (3) an operator go. Catalog mechanics (preset
file, manifest, revision bump, tests) are deliberately deferred to
launch time — the YAML here is the draft of record.

## Sequencing the operator set

Bring communication overhead down FIRST, then dogfood. Standing this
team up before the overhead work would make five more seats pay today's
messaging tax while building its replacement.

## Phase 0 — communication overhead reduction (pre-team, bounded lanes)

The measured items, strongest first (citations: threads memo = mesh-
task-threads-research.md, overhaul study = mesh-messaging-overhaul-
research.md; both on the wave-1 archive):

1. **Versioned onboarding / recovery card** — 94.3% of all exactly
   duplicated text was onboarding replayed to the same seat (53,508 of
   56,768 surplus characters). A versioned card delivered once per
   context generation, not per nudge/relaunch, attacks the single
   largest measured redundancy.
2. **One combined assignment rendering** — nine assignments each
   delivered twice (authored contract + generated card): consolidation
   ceiling 8.9% of task text. Merge into one rendering that preserves
   every contract field and the assignment token.
3. **No-response convention for info-only traffic** — `INFO ONLY:`
   notices that wake a model into generating a reply are pure overhead;
   an empty delta check must not cost a turn (threads memo's read-
   economics note; delivery standard already has the prefix vocabulary).
4. **Lead notice dedup** — the wave-1 lead's ~1.29B cache reads were
   attributed to accumulated notices; the same event visible through
   inbox, workflow event, and task metadata is ONE event with several
   sources, not three notices (overhaul study's receipt/dedup rules).

5. **Ledger as append-log with rendered projections** — operator
   observation, measured 2026-09-07: 283 of 694 commits on taurjob
   (40.8% of the whole project history) touch a ledger file; the lead
   spends a large share of its output re-rendering state into markdown
   by hand, with git as a poor man's event log. Design: ledger entries
   become appendable events through a mesh verb (`entry`, `amend`
   superseding a prior id with a reason, tombstone with a reason —
   never an in-place edit); "the ledger" is a fold over the stream;
   `ledger.md` becomes a GENERATED rendering produced on demand and
   committed only as point-in-time snapshots at real boundaries (wave
   closure, milestone freezes). No service, no watcher — the ledger is
   pull-only, which is exactly why it needs none of the messaging
   study's delivery machinery. Most candidate events already exist as
   structured records (tasks, rulings, completions, budget raises);
   the new event type covers what is prose-only today: honest
   outcomes, remaining, evidence links. This is also the gentlest
   rehearsal of the mesh-next pattern (append + fold + render) with
   zero transport risk.

Each is a bounded mesh or machinery lane in the existing ad-hoc mode.
None requires the new storage; all survive it. Run them post-wave (mesh
0.2.29 stays locked while the wave is live), measure against the
threads memo's baselines, and only then consider the team below.

## Roster (five seats — draft, revisable by the wave-2 retro)

Taurhaus-core blueprint shape, adapted: mesh is concurrency, protocol,
storage-recovery work — the class where the depth leader reviews and
the cross-file leader architects.

| Seat | Model / effort | Duty |
|---|---|---|
| Lead | Fable, high | Orchestration, decomposition, acceptance; owns the stage gates of the migration plan |
| Architect | Astra, xhigh | Holds the charter below; adjudicates design questions across lanes; no implementation |
| Implementer | Astra, high | The single standing implementation lane, diff-budgeted as always |
| Altitude reviewer | Fable, high | Claude-family review of GPT-authored code (primary route — most authoring here is Astra) |
| Judge | Astra, high | GPT-family review of Claude-authored artifacts, design docs, and evidence audits |

No standing asset seat (borrow taurjobs' pattern on demand); Sol stays
overflow; one review round per lane by default.

### Preset draft (catalog mechanics at launch time)

```yaml
preset_id: mesh-core
name: Mesh Core
description: "Five seats for mesh-next: Fable leads and reviews at
  altitude, Astra architects under the mesh-next charter and implements
  one diff-budgeted lane, the Astra judge covers Claude-authored
  artifacts. Sol is overflow only."
version: "0.1.0"           # draft; becomes 1.0.0 at launch
lead_role_id: v3-lead-claude
agent_slots:
  - role_id: astra-architect
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: mesh-architect
      reasoning_effort: xhigh
      # charter override: see "Architect charter" — delivered as the
      # slot's context/instructions augmentation at launch
  - role_id: astra-heavy-implementer
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: implementer
  - role_id: fable-altitude-reviewer
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: altitude-reviewer
  - role_id: judge-astra
    count: 1
    project_binding: lead_project
    overrides:
      name_pattern: judge
defaults:
  team_name_pattern: "{project}-mesh-core"
  tmux_layout: tiled
```

## Architect charter (slot-level, on top of astra-architect)

The architect's authority IS the recorded design corpus, in precedence
order:

1. `docs/design/mesh-task-threads-research.md` — read-side law: bounded
   task-evidence projection; no default subscriptions; shared
   consumption must clear the measured bar (1.68×–37×); DMs and
   directed delivery retained.
2. `docs/design/mesh-messaging-overhaul-research.md` (+ its operator
   decisions addendum) — write/transport law: one segmented append-only
   team journal as eventual message authority; one scheduler in the
   team daemon; per-harness adapters behind a common claim contract;
   send-keys as guarded fallback; the six-stage migration with its
   rollback boundaries; the evidence-label vocabulary (S/D/P/I/U).
3. `docs/design/native-push-probe-report.md` — what the installed
   frontier harnesses actually support; Claude Code + Codex native push
   is first-class, agy/grok stay on the fallback (operator decision,
   ~80% frontier-duo assumption).

Standing rules:

- **Deviations from the corpus are spec-deltas**, written and operator-
  visible, never silent. The memos' open questions are the architect's
  backlog; answering one updates the corpus, not just the code.
- **Stage gates are real**: no stage of the migration begins before the
  prior stage's advance-evidence exists; the architect signs each gate
  in the ledger. Version contracts (mesh protocol/schema, taurhaus
  daemon protocol, the messaging-format marker) change only through the
  paired-contract review the study specifies.
- **Never self-ruled**: a finding implicating the architect's own
  design decision escalates to the lead and the altitude reviewer —
  same clause as the product team's architect.
- **Measurement before machinery**: any claimed overhead saving is
  measured against the threads memo's baselines with its estimator
  disclosed; reliability work justifies itself with retention, routing
  correctness, and honest receipts, not token claims.

## Dogfooding as telemetry (binding when the team launches)

The team runs on mesh while rebuilding mesh's messaging. Every
communication failure it experiences — a lost body, an interleaved
prompt, a stale instruction consumed, a wait misread — is recorded as a
structured field observation (what, when, which mechanism) in
`docs/mesh-next/field-observations.md`. The wave-1 archive became the
threads memo's dataset by accident; this team's dataset is deliberate.
The lead ensures observations are captured at the moment of failure,
not reconstructed at retro time.

## Launch checklist (when the operator says go)

1. Wave-2 retro roster lessons reviewed against the five seats above.
2. Phase-0 overhead lanes landed and measured.
3. Catalog lane: preset file + manifest + revision bump + hash/test
   pins + docs (the standard mechanics; one small lane).
4. Charter delivered as the architect slot's augmentation; corpus files
   verified present at their cited paths.
5. Usage-window check: no concurrent frontier wave competing for the
   same Claude/Codex subscriptions.
