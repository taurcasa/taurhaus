# Mesh-next phase 0 — execution ledger

Orchestrator (Fable, account2 session), started 2026-09-08 from
`mesh-next-session-handover.md`. This is the running record of phase-0
lanes: what ran, on which model, what it returned, how it was verified,
and what was adjudicated. Verdicts here bind the lane outputs the way
`mesh-next-adjudication.md` binds the studies; the corpus's next revision
folds them in. Labels follow the corpus (S/D/P/I/U).

Model split in force (operator, 2026-09-08): Fable orchestrates and
adjudicates; every implementation and research lane runs Codex
`gpt-6-astra` at reasoning effort `high`; Opus holds the decorrelated
review lens on GPT-authored work; Astra `xhigh` is reserved for an
architect or lead-auditor seat; never Sonnet; Sol overflow only.

## State at start

- Corpus frozen (`mesh-next-adjudication.md`). Phase 0 not started.
- `fix/telemetry-contracts` (M1–M3 repair) belongs to the outgoing
  default-account session; at takeover it had 3 commits ahead of main,
  no PR, and its own Codex implementer plus a `cargo test` still running.
  Hands off. Every telemetry-based measurement stays gated on it AND on
  a wave-2 mesh state archive, which does not exist yet (the taurjob
  wave-2 team is still standing on the account2 root).
- Mesh locked at 0.2.29 (`6789201c`, protocol 1 / schema 1). **Rule
  derived here (S):** `scripts/resolve-mesh-binary.sh` rebuilds from
  `~/projects/mesh` whenever the workspace binary's `git_commit` differs
  from the lock, and `mesh-verify-lock` then fails — so any commit on
  mesh master past the lock commit breaks the taurhaus platform-build
  gate until a deliberate lock-flow release. Every mesh lane therefore
  lands on a branch in a sibling worktree; master moves only inside the
  release.
- Wave-1 population re-verified at takeover (S): ledger.md
  `d9272cf6…`, rulings.md `c08329fb…`, archive tarball `60bd9ba9…` —
  identical to the ledger study's captured hashes. Wave-2 ledger.md was
  `a19e3c6d…` (85,077 bytes) at 03:5x UTC and is still live.

## Lanes

| # | Item | Brief | Procedure / model | Output | Status |
|---|---|---|---|---|---|
| A | Ledger increment A — offline existing-record reader | `ledger-reader-brief.md` | `feature-pr` in `~/projects/mesh-ledger` (`feat/ledger-reader` off master `6789201c`), implementer `gpt-6-astra` high, Opus conformance + operational lenses | mesh branch commits `d39c397`, `ace395e` (+ any fix-round commits) | implementer turn complete; orchestrator verification PASS (below); procedure review/gate in progress |
| D1 | Artifact-as-event authoring intake design | `ledger-artifact-as-event-brief.md` | research-sweep-long, `gpt-6-astra` high | `.check-logs/mesh-next-phase0/ledger-artifact-as-event.md` (70,739 B) | returned `ok`; Opus lens pending; adjudication pending |
| D2 | One combined assignment rendering design | `assignment-rendering-brief.md` | research-sweep-long, `gpt-6-astra` high | `.check-logs/mesh-next-phase0/assignment-rendering-design.md` (82,375 B) | returned `ok`; Opus lens pending; adjudication pending |
| D3 | Versioned onboarding / recovery card design | `onboarding-card-brief.md` | research-sweep-long, `gpt-6-astra` high | `.check-logs/mesh-next-phase0/onboarding-card-design.md` (56,547 B) | returned `ok`; Opus lens pending; adjudication pending |
| D4 | Info-only no-response + lead notice dedup design | `info-only-and-notice-dedup-brief.md` | research-sweep-long, `gpt-6-astra` high | pending | running |
| B | Ledger increment B — proposition-to-source gap matrix | `ledger-gap-matrix-brief.md` | research-sweep-long, `gpt-6-astra` high, on A's candidate binary | pending | queued behind A's procedure result |

## Lane A — orchestrator verification (S, 2026-09-08 ~04:3x UTC)

Candidate: `~/projects/mesh-ledger/target/debug/mesh`, reporting
`0.2.29+ace395e6c362f8fa0b69e9d6dbc4f5b38d6fe3f8`. Input: the bundle
assembled at `.check-logs/mesh-next-phase0/wave1-bundle/` from the
archive (tarball `taurjob-team/{config.json,state}` beside a copy of
`tasks/*.json` and `telemetry/*.jsonl`); journal digests `907075d8…`,
`0f26b0e3…`, `c5f3ce38…` match the study. Every command below ran with
`env -i` and no `--claude-dir`.

| Check | Result |
|---|---|
| `ledger census --format json` vs ledger study §2.3/§11 | journal rows 321/53/98; 11 snapshots; 23 rulings; 11 status agreements; `budget_raised` rulings 0; `idle_monitor_records` 0; exactly one budget-related ruling (task 11 seq 1 `budget-430`, no field); event types 11 created / 12 assigned / 1 superseded / 72 state / 7 completed / 7 review / 5 progress / 23 ruling (+8 accepted, 13 started, 2 lead-admin, 36 delivery, 37 read, 86 sent, 1 blocked = 321); window 19:34:32.013–20:11:26.180 UTC; 4 accepted cores with the study's event IDs and refs; 0 source conflicts. **Exact match.** |
| Negative cases | T1/T2 `in_progress` with rejected verdicts; T7 and T9 completed with no verdict on their own task (T9's completion names the T2 rejection); T10 `in_progress` with the `b825b54` accepted ruling; T6 completion candidate `06ffc9b` retained. Pinned in `tests/ledger_wave1_archive.rs` from archive bytes, not the hand ledger. |
| Determinism | two `render --format markdown` runs and two `--format json` runs: byte-identical digests. |
| Pull purity | bundle tree digest identical before and after; no new files; output on stdout only. |
| Live-root refusal | `--claude-dir` with the offline verb → exit 1, "offline reader refuses --claude-dir/--team/--name and their environment defaults". |
| Manifest | every bundle file digested with byte length and row count; `telemetry/*` listed `consumed: false`; layout version 0; reader version; `consistency: vector_cut`; typed gaps (`retention` vs `schema`) with the study's vocabulary. |
| Diff budget | 1,570 inserted lines (Cargo.lock excluded) of 2,200; `sha2` the only new dependency; `just ledger-archive-check ARCHIVE` recipe present. |
| Lane gate (reported by the implementer) | `just check-quick`, `just lint`, `just test` (688 passed, 1 ignored), archive check passed. Deviation recorded: one early cargo run proceeded despite another cargo job; later runs waited. |

Orchestrator notes for the procedure's review: (minor) the per-task
evidence blocks in the Markdown are escaped JSON dumps of the contract
and assignment history rather than rendered lines — acceptable for the
offline reader, to be replaced by increment D's canonical compact
Markdown; the `<details><pre>` manifest block is deterministic but not
human-diffable. Neither affects the advance condition.

## Findings surfaced by the design lanes (for the mesh/taurhaus backlog)

- **Manual nudge contradicts a recorded wait (S, D3 probes C24–C26):**
  `mesh nudge --owner <seat> --task <id>` renders "ACTION REQUIRED: Resume
  task #N now … Start now: <first step>" while `metadata.awaiting_go`
  still holds the assignment token. The M2 repair covers the taurhaus
  deadline predicate; mesh's own manual nudge path needs the same
  wait-awareness. Regression fixture candidate.
- **Effort relaunch replays full onboarding (S, D3 inventory):**
  `pipelines/effort.rs` stops and re-enters the canonical resume pipeline,
  which sends onboarding again — a context-generation event that the
  versioned card must classify as one baseline, not a replay.
- **GO release delivers no assignee body (S, D2 probe 37):** `task update
  --go` clears `awaiting_go`, journals, notifies the lead; the owner's
  inbox is unchanged. The historical T9 GO carried a changed rubric (60
  checks vs 57), so a GO rendering must carry deltas.
- **Combined assignment card is a correctness change, not a token
  saving (S, D2):** a field-preserving combined rendering of the nine
  wave-1 pairs is 961 characters (1.14% of task text) smaller than the
  authored+card pair, against the study's 8.9% ceiling — the pairs'
  prose was mostly unique. The design's value is one operative contract
  with an explicit wait, not bytes.
- **Onboarding replay is an interval, not a number (S/U, D3):** all 15
  wave-1 onboarding bodies have `tmux_injected` receipts and 13 have read
  joins, but the workflow journal carries no launch/resume/compaction
  event, so none of the ten extra copies can be classified as necessary
  recovery versus replay. Reachable saving on that sample: 0–13,377 TE.
  The brief's "28,916.50 TE fixed onboarding baseline" was wrong: that
  figure is onboarding + broadcasts + cross-task reports; onboarding
  alone is 19,925 TE.
- **No committed artifact carries front matter today (S, D1):** 0 of 92
  wave-1/wave-2 review files begin with front matter or `RESULT`; 86
  exceed 4 KiB; the seven retained completion summaries (131–572 bytes)
  already carry outcomes and qualifications.

## Adjudications

(Filled per lane after the Opus lens returns; see the sections below as
they are added.)

### D2 — combined assignment rendering: round-1 review outcome

Opus defect lens (2026-09-08, ~10 min, 46 tool uses): **fix_required** —
1 blocker, 5 majors, 5 minors, 2 nits, 4 open questions. Blocker: the
nine "field-preserving" re-rendered cards silently paraphrased five
archived `mesh` commands (including the candidate-referenced ruling
command) because the lane's own `verify-report.py` asserted every
backticked `mesh` string had been executed. Majors: a renderer-invented
hold with no lifecycle record (a second wait authority); exemplars not
conforming to the template; `mesh task assign` prints the inbox message
id, not the assignment token (an [S] claim reproduced false, and a real
design gap); mesh's `parent_task_ids` / `scaffold_class` / `sunset_*`
fields unmapped; `assignment_context` lifetime across generations
undefined versus `ASSIGNMENT_CLEAR_KEYS`. Findings file:
`.check-logs/mesh-next-phase0/d2-opus-findings.md`.

**Orchestrator ruling ("is this worth another round?"): yes, one bounded
fix round.** Value: the blocker is an evidence-integrity defect in the
deliverable's central demonstration and every major is a concrete,
cheap document correction; the design's structure and measurements are
otherwise sound (all other spot-checked [S] citations reproduced).
Stopping condition: findings 1–6 addressed and orchestrator-verified
directly (the five commands restored verbatim, the hold rule replaced by
report-only plus a lifecycle BLOCKED route, the assign-output token
surface added, the clear-keys rule specified); no third round. The fix
lane runs on `gpt-6-astra` high through the same research procedure,
editing the document in place.

### D1 — artifact-as-event intake: round-1 review outcome

Opus defect lens (2026-09-08, ~9 min, 39 tool uses; 14 [S] claims
spot-checked, all reproduced): **fix_required** — 1 blocker, 5 majors,
5 minors, 3 nits, 2 open questions. Blocker: the validate-first order in
SD1 lets an optional ledger annotation abort a mandatory delivery (no
opt-out, no degraded path) — the 9/9 seat invariant. Majors: whole-
artifact digest inside the idempotency key (false conflicts on growing
NOTES files); `entry_key.wave/scope` required to be frozen IDs while the
only example authors labels and no source is named; reference digest
required/optional contradiction with no normalization spec; the section
selector is an unlabeled spec-delta that supersedes the amendment's
"literal prose body below front matter" shape; `--artifact` read
semantics undefined. Findings file with the orchestrator's binding
directions: `.check-logs/mesh-next-phase0/d1-opus-findings.md`.

**Orchestrator adjudication folded into the fix directions (binding):**
source-commit-first failure contract (lifecycle commits as today; ledger
validated and admitted after; rejection = source receipt + ledger error
+ nonzero exit naming the standalone idempotent repair); the durable
submission-receipt store and the task-writer idempotency change are
DROPPED (SD1 = adapter call + `--summary-file` only); idempotency
equality is the event's own normalized content, artifact digest is
provenance; the adapter derives wave/scope from the frozen assignment;
mesh computes every digest; SD5 named, with "everything after the front
matter is the body" retained for dedicated entries; `.md`-only
attachment sniffing, never fatal unless a parsed `ledger` namespace is
invalid; SD3 retention archives at the boundary snapshot, not at ingest;
increment C boundary fixed (adapter + validation + references + writer/
fold/CAS + submission call; no receipt store, no export enforcement).

**Ruling ("is this worth another round?"): yes, one bounded fix round**
on `gpt-6-astra` high editing the document in place; stopping condition:
findings 1–6 and OQ-A/OQ-B implemented as directed and orchestrator-
verified; no third round.

### D3 — versioned onboarding / recovery card: round-1 review outcome

Opus defect lens (2026-09-08, ~12 min, 70 tool uses; every [S] claim
reproduced, the archive census re-derived byte-for-byte, five renderer
call sites confirmed): **fix_required** — 0 blockers, 6 majors, 2
minors, 2 nits, 3 open questions. Majors: baseline obligation key
inconsistent with the view-varying `delivery_id`; correction coalescing
needs state no existing record can hold; trimming `render_role_sections`
would rewrite every exported Claude agent definition (`just
export-agents` shares the body); suppressing reonboard removes the only
manual recovery path (Antigravity has no compaction hook; `/clear`
mints no generation); "admitted" compaction boundary ambiguous for
`Skipped` deliveries; root-move rule cites the wrong module
(`team_move.rs` relocates the whole tree, receipts included). Findings
with binding directions: `.check-logs/mesh-next-phase0/d3-opus-findings.md`.

**Orchestrator adjudication folded into the fix directions:** three
fixed keys (obligation = recipient+context; `delivery_id` = card_key +
kind, stable across recomposition; content revision on the receipt);
coalescing and per-revision budgets DROPPED (latest operative version
wins, one attempt + one retry per delivery id); a separate steering
renderer so `render_role_sections` stays byte-stable; an
operator-authorized `reonboard --force` that mints a generation and is
logged; "admitted" = delivered-or-recorded with a `pending` obligation
satisfied by the next delivery; root-move restated against
`team_move.rs`.

**Ruling: yes, one bounded fix round** on `gpt-6-astra` high in place;
stopping condition: majors 1–6 implemented as directed and
orchestrator-verified; no third round.

### Lane A — closure

`feature-pr` returned **complete** (2026-09-08 ~04:41 UTC): implementer
`gpt-6-astra` high, two commits (`d39c397`, `ace395e`), Opus conformance
lens **approve** (9 findings: 3 minor, 6 nit) and Opus operational lens
**approve** (9 findings: 3 minor, 6 nit), gate **pass** (`just
check-quick`, `just lint`, `just test` 688 passed / 1 ignored, `just
ledger-archive-check`), no fix round. Orchestrator verification above
was independent of the procedure. Increment A's advance condition
(reproduce §2 counts and negative cases; inputs unchanged) is met.
Branch `feat/ledger-reader` in `~/projects/mesh-ledger` is the base for
increment C; it is NOT merged to master (lock rule).

Minors carried as increment C/D follow-ups (not blocking A):
- workflow event types re-derived as strings instead of matching the
  typed `WorkflowEvent` authority; a second task-id ordering beside
  `src/tasks.rs`'s — C should reuse the typed enum and the one ordering;
- the offline verbs hard-fail when `MESH_TEAM`/`MESH_NAME`/`CLAUDE_DIR`
  merely exist in the environment (a real pane always has them): the
  verbs should IGNORE ambient defaults and refuse only explicit flags;
- bundle-path IO errors do not name the path the user passed; an
  unknown/newer `eventType` aborts the bundle with an indistinguishable
  message (fail-closed is correct; the diagnostic must name the cause);
- Markdown: the manifest `<pre>` block and the per-task JSON blocks are
  single multi-KB physical lines — replaced by D's canonical compact
  Markdown; `reader_version` should carry `git_dirty`.

## Contract-repair gate — CLOSED by the outgoing session (S)

PR #148 `Telemetry/wait contracts: consume what mesh actually writes
(M1–M3)` merged to main at 2026-09-08 02:40 UTC (04:40 local) as
`90b326e5` (13 files, +883/−36): binary-produced fixtures
(`scripts/mesh-contract-fixture.py`, `mesh_contract_fixture.rs`), the
monitor-record import with message-id dedup (`routing_report/mesh.rs`),
the assignment-token GO predicate with boolean compatibility
(`stores/mesh_task.rs`), the deadline-pass wait handling, and
`docs/operations/telemetry-contracts.md`. The outgoing session rebased
this program's briefs commit on top (`9dd8427e` → `eeaa5d50`); local
main equals origin. The contract-fix worktree is gone. M3's historical
cause for task #31 stays an open check (`M3-WAVE2-31`) that only a
closed wave-2 export can settle.

Consequence for phase 0: the instruments are real, so the measured
items (onboarding replay census, notice dedup, info-only turn counts)
are now gated ONLY on the wave-2 mesh state archive, which does not
exist yet and belongs to the operator / the standing taurjob team. The
`mesh_task.rs:247` fixture that writes boolean `awaiting_go = true` is
the intentional backward-compatibility case of the merged predicate,
not a defect (D2 lens OQ3 disposed).

### D1 — adjudication: ACCEPTED (design of record for increment C)

Round-1 fix verified directly by the orchestrator against
`d1-opus-findings.md` (2026-09-08 ~04:50 UTC): every binding direction
is implemented in the text — source-commit-first failure contract with
the split receipt and standalone idempotent repair (§1 SD1); the
submission-receipt store, coordinator lock and task-writer idempotency
change appear only as DROPPED; `.md`-only attachment sniff, never fatal
unless a parsed `ledger` namespace is invalid; `adapter_version` with
`schema_version`/`audience_ref` forbidden as authored fields; the seat
authors `{kind, slot}` and mesh derives wave/scope on the submission
route; mesh computes every digest, with a per-kind reference table and
a published canonical normalization; SD5 keeps "everything after the
front matter is the body" for dedicated files; SD3 retention is
`referenced` at ingest and `archived` only at a boundary export; SD6 for
the expected head; `mesh ledger render --view current|narrative`; three
seat rules; increment C boundary exactly as ruled. Eight mesh probes
re-run in a tempdir (exits 0,0,0,0,0,2,2,2). Promoted to
`docs/design/ledger-artifact-as-event.md` with its evidence sidecars
under `docs/design/evidence/ledger-artifact-as-event/` (the extracted
archive copy is not committed; the archive hash is its identity).

Spec-deltas accepted with it (SD1 adapter call + `--summary-file`;
SD2a bounds / SD2b atomic batch / SD2c `invalid_input`; SD3 boundary
retention; SD4 `audience_ref`; SD5 trailing body + section selector;
SD6 expected-head transport) become increment C's contract; the
ledger study's next revision folds them in.
