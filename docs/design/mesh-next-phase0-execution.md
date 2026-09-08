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

### D3 — adjudication: ACCEPTED (design of record for the onboarding / recovery card lanes)

Round-1 fix verified directly by the orchestrator against
`d3-opus-findings.md` (2026-09-08 ~04:58 UTC): three fixed keys
(`obligation_key = (recipient, context)`, `delivery_id = hash(card_key,
delivery_kind)` stable across recomposition, `content_revision` as a
receipt field); coalescing, replaced-revision ranges and per-revision
budgets dropped for latest-version-wins with one attempt plus one retry
per delivery id on the single runtime pointer; a separate
`render_card_steering` with `render_role_sections` byte-stable for the
agent export and a new golden; operator-authorized `reonboard --force`
minting a generation and logging `onboarding.generation.forced`,
excluded from the dedup fixture, Antigravity's manual path; "admitted"
= delivered-or-recorded with a `pending` obligation satisfied by the
next delivery; root move restated against `team_move.rs` (whole tree
travels; pre-/post-flip root tuple rule); byte cap tied to the minimal
steering card and left UNVERIFIED until the bundled-role fixture; three
role rules pointing at the card; `accepted` as the only durable-append
receipt; citations fixed; OQ-A/B/C settled with safe defaults. 28 mesh
probes re-run in a tempdir. Promoted to
`docs/design/onboarding-card-design.md` with evidence under
`docs/design/evidence/onboarding-card/`.

Implementation shape (not launched in phase 0): two bounded lanes —
taurhaus (card schema, activation generation, steering renderer, forced
reonboard, pending skipped-compaction recovery, deadline-nudge content)
and mesh (assignment/wait/correction projection, delivery identities,
receipts) — with joint fixtures at the existing state boundary. The
ordering decision stays with the operator: it touches the live
compaction path and both binaries' release trains.

### D2 — adjudication: ACCEPTED as a correctness change; overhead item 2 RECLASSIFIED

Round-1 fix verified directly by the orchestrator against
`d2-opus-findings.md` (2026-09-08 ~05:05 UTC): the five archived
commands are back verbatim and the verifier's executed-command
assertion is scoped to the executed-evidence section; the renderer
reads `awaiting_go` and declared blocks and creates no hold (field
conflicts → visible annotation + the lifecycle's BLOCKED route); all
nine cards plus a synthetic optional-section card go through the one
literal template; `mesh task assign` is correctly described as printing
the inbox message id, with a labelled `assignment_id` added to the
assign output/JSON envelope; `parent_task_ids`, `anchor`,
`scaffold_class`, `sunset_*` and the archived ruling/verdict/artifact/
review fields are in the matrix with an unknown-key rule; every
`assignment_context` subkey has a declared lifetime and the exact
top-level clear entry; writers are named per subkey; the GO release
moves to a proposed compare-and-commit `task go` command with its own
bounded body (the legacy `task update --go` stays unguarded pending
migration review); spec-deltas are listed with owners; line 5 keeps the
standard's scope; the historical T9 GO is rendered through the release
template (candidate `8f97e28`, rubric `6a44056`, 57→60 checks, R1→R10,
47(d) deferred). 41 mesh probes re-run in a tempdir. Promoted to
`docs/design/assignment-rendering-design.md` with evidence under
`docs/design/evidence/assignment-rendering/`.

**Program-level finding (S, measured with the threads study's
estimator):** a field-preserving combined rendering of the nine wave-1
pairs is LARGER than the authored contract plus generated card it
replaces — verbatim doctrine −2.99%, linked-not-pasted doctrine −4.81%
of the 84,524-character task-text denominator — against the study's
8.9% ceiling. Overhead item 2's token premise does not survive contact
with the preserved fields. The change stays justified by correctness
alone: one operative contract with an explicit token-bound wait, a GO
release that carries its deltas (the real T9 GO changed the rubric),
generation-scoped context that cannot leak across reassignment, and
the assignment token visible where the lead needs it. Implementation
priority follows that value, after the ledger increments.

### D4 — info-only no-response + lead notice dedup: round-1 review outcome

Opus defect lens (2026-09-08, ~13 min, 58 tool uses; every probe
re-executed, the archive census reproduced exactly, all twelve mesh
`append_message` sites confirmed inventoried, M1 identity reuse exact):
**fix_required** — 1 blocker, 5 majors, 5 minors, 1 nit, 4 open
questions. Blocker: "GO still wakes" asserted while mesh writes no
member-directed GO notice (only the lead is notified; no generated GO
kind). Majors: 8 of the 10 measured `INFO ONLY:` bodies changed a work
rule (two branch corrections, a task withdrawal, five copies of a
standing rule), so indefinite deferral is the design's principal risk;
the deferred array vs writer-side supersession is unspecified; rejecting
`ACTION REQUIRED … --priority low` is a breaking change under an
additive claim; "one predicate" spans two codebases with no shared
vectors and misses `reinjection.rs:173`; the design forgoes the
suppression the existing daemon filter already supports. Findings with
binding directions: `.check-logs/mesh-next-phase0/d4-opus-findings.md`.

**Orchestrator adjudication folded into the directions:** GO as a
message is out of scope (the generated release body is D2's `task go`;
an authored prose GO is never `info`); a mechanical send-time lint
rejects `INFO ONLY:` bodies carrying supersession/withdrawal/release
language, plus lead-visible undrained-info counts; generated
kind-bearing notices are never `info` and info never supersedes; the
low-priority action combination is suppress-and-label on the unchanged
path; one committed expectation VECTOR TABLE owned by mesh and mirrored
byte-for-byte in taurhaus tests; and a STAGE SPLIT — stage 1 (phase-0
deliverable, no new store): expectation-aware suppression in the
existing daemon filter, terminal key selection and cron gate, the lint,
the vectors, covering Codex/Antigravity/Grok now; stage 2 (opt-in,
Claude, later): the deferred array, gated on the sanctioned-earned-read
ownership and a reviewed version transition.

**Ruling: yes, one bounded fix round** on `gpt-6-astra` high in place;
stopping condition: blocker and majors 2–6 implemented as directed and
orchestrator-verified; no third round.

### Lane B — gap matrix: returned `unavailable` (failed closed, honestly)

The Astra lane (2026-09-08 04:41–05:02) verified the three pinned
wave-1 hashes, built the candidate reader, reproduced the §11 census
(29 rows, 52,159 cell characters, the trial projection digest) and
wrote `ledger-gap-matrix.md` (1.23 MB): 36 checked short-result units,
56 selected observations across the deep cases (hero, T25, T26,
T27/T28, governance, live wave-2 capture), the seven malformed-width
rows and the 5,881-character paragraph as renderer fixtures, a
provisional §4 field disposition and named spec-deltas — and 3,726 of
3,777 candidate fragments UNRESOLVED. Its own verdict: "Fail this lane
closed. Do not freeze payload v1 from this incomplete report." The
exhaustive proposition matrix the brief asked for does not fit one
lane turn; the researcher chose fail-closed over guessing, which is the
corpus's rule. Opus lens on the partial document in progress;
orchestrator disposition follows it.

### B — acceptance-owner rulings on the ambiguous codings and the payload-v1 provisional freeze

Rulings (orchestrator as acceptance owner, 2026-09-08 ~05:10 UTC), each
consistent with the study §4/§5, the adjudication and the accepted
intake design; none adds a payload kind:

| Ambiguity | Ruling |
|---|---|
| A1 measure/diagnose display vocabulary | Retain source `work_kind`; any display adapter is a renderer concern; no ledger field. |
| A2 `remaining: none` on short rows | Never implicit: an explicit `outcome.remaining_status` from an authorized result, else rendered unknown (as the intake design already says). |
| A3 accepted-candidate join | An attributed evidence association in the ledger; the typed candidate/review relation belongs to the ruling authority (with SD-SOURCE-TUPLE roles), not the ledger. |
| A4 T6a alias | An explicit alias inside the scope reference; never an unauthenticated task. |
| A5 late-task retention vs artifacts | Retention gap; resolved only by the closed exports; the artifact arm stays a separate source class. |
| A6 ruling artifact vs new declaration | The artifact's front matter IS the event (intake design); no per-fact file. |
| A7 R36/R38 amendment vs observation | Correction of interpretation = `amend`; a changed instrument/population = a new slot with a `prior_observation` reference (study §2.5 rule 1). |
| A8 E5 causal correction / override | Lead override with reason and retained original attribution (study §4 authority table); the 59-second assertion stays U. |
| A9 budget typed fields | Ruling/task authority (SD-BUDGET-AUTHORITY); the ledger only references the ruling. |
| A10 T26 PASS scope | Assessment and limitation per scoped reference (SD-CLAIM-ASSESSMENT); no document-wide Boolean. |
| A11 T28 closed vs fulfilled | `outcome.scope_disposition: partial` plus `remaining` items with the counting basis in the referenced result. |
| A12 wave-2 B1 table | A separate captured stratum in any future census; renderer scope. |
| A13 malformed fragments / stale cells | Renderer fixtures with schema-owned columns (accepted as requirements). |
| A14 mirrors vs logical acts | Continuation work on the closed exports; no payload effect. |
| A15 review independence / waiver | `audience_ref` (SD4) plus a `note` with `qualifies` for a disclosed waiver; projection enforcement is increment D's. |

**Payload v1 — provisional freeze:** the study's §4 four kinds with the
intake design's field spellings; reference roles gain `baseline`,
`gated`, `reviewed`, `landed`, `red_base`, `red_features`; `retention`
gains `retain_until`; assessment is per scoped reference; no new
kinds; budget and diff-confirm typing go to the ruling authority.
"Provisional" is honest: the exhaustive proposition census (3,726
syntactic candidates unresolved; semantic denominator U) continues as
a background lane when the closed wave-1/wave-2 exports exist, and can
only ADD evidence for fields already present or move a typed relation
into the ruling authority — it cannot remove a correction/retention
mechanism the study requires. This freeze feeds increment C1; the
Opus lens on the partial matrix gates the launch.

### D4 — adjudication: ACCEPTED; stage 1 is a phase-0 mesh lane

Round-1 fix verified directly by the orchestrator against
`d4-opus-findings.md` (2026-09-08 ~05:15 UTC): mesh generates no
member-directed GO notice today (S, `main.rs:3316–3333`; the release
body is D2's `task go`), and an authored GO/release/withdrawal/
correction is never classifiable as info; the 8/10 measured base rate
is the principal risk, met by a mechanical send-time lint extending
`lint_and_prepare_send_message` (bounded keyword list, no override,
direct and broadcast) and per-member undrained-info counts on `mesh
who`/read surfaces; generated kind-bearing notices are never info and
info never supersedes anything, with the two fixtures corrected;
`ACTION REQUIRED … --priority low` stays accepted on the unchanged path
as suppress-and-label; one committed expectation vector table owned by
mesh (`tests/fixtures/expectation-vectors.json`) mirrored byte-for-byte
into taurhaus, the direct reinjection writer inventoried as
action/recovery; STAGE SPLIT — stage 1 (existing daemon filter,
terminal key selection, cron gate, lint, counts, optional field with
prefix fallback, vectors; no new store; feature-disabled behavior
explicit) and stage 2 (opt-in Claude deferred array, gated on the
earned-read ownership and a reviewed version transition; merged-read
order/budget/overflow/watch specified); cron reminders are action;
corpus receipt names retained with the two additions marked; the
message-convention sentence retained and rules appended; stop out of
scope; the old-binary field-stripping case observed (C31) and handled
by the optional field with prefix fallback. 31 probes re-run. Promoted
to `docs/design/info-only-and-notice-dedup-design.md` with evidence
under `docs/design/evidence/info-only-and-notice-dedup/`.

### B — round-1 lens outcome and ruling

Opus defect lens on the partial matrix (2026-09-08, ~13 min, 53 tool
uses; census re-run byte-identical; every population number
recomputed; all 43 retained-structured codings machine-verified with
0 failures; sampled deep cases hand-verified; no invented claim):
**fix_required** — blocker (coverage 51/3,777, R1/W2 uncoded),
majors: headline numbers unreconciled (15 verified hero-stratum
codings unreported); T8's `remaining: none` contradicted by its own
retained completion limitation ("not validated") and coded with
boilerplate; spec-deltas not reconciled with the accepted intake
design (SD-RETENTION-REF = SD3, REVIEW-INTEGRITY ≈ SD4, CLAIM-
ASSESSMENT = accepted vocabulary, SOURCE-TUPLE role names collide with
the study set) — partly the orchestrator's brief, which predates D1;
§4 disposition rows marked "necessary" on link-gap evidence that
evidences roles, not authored fields; no field reportable as unused.
Findings with directions: `.check-logs/mesh-next-phase0/d5-opus-findings.md`.

**Orchestrator ruling:** the payload-v1 provisional freeze STANDS with
two corrections — reference roles: the study set governs, NEW roles
`baseline`, `gated`, `reviewed`, `red_base`, `red_features` (`landing`
already covers "landed"); and the T8 contradiction becomes a C1
acceptance fixture (both written into `ledger-writer-brief.md`).
Increment B stays OPEN: one bounded fix round makes the partial matrix
honest and reconciled (retitled PARTIAL, coded = 51 with strata, the
15 promoted, T8 recoded, SD-* re-expressed against SD1–SD6, per-field
rows with classes, citations fixed); exhaustive coding waits for the
closed exports. C1 launches now; B's fix round runs in parallel.

### Incident — the gap-matrix lane resumed itself and overwrote the reconciliation (procedure defect, 2026-09-08 05:03–05:33 UTC)

The `research-sweep-long` copy carries `resume: true` (added so a
design study could span several turns). When the gap-matrix researcher
returned `status: unavailable`, its Opus wrapper treated the run as
unfinished and launched `codex exec resume` turns (turn0 at 05:03,
another still running at 05:31) that attempted the exhaustive coding
the orchestrator had explicitly declined, and rewrote
`ledger-gap-matrix.md` (2.1 MB, "3,609 coded propositions", 2,254 of
them labeled U) at 05:31:54 — one minute after the reconciliation fix
round (05:21–05:30) had written the directed PARTIAL document to the
same path. The fix round's staged copy and the original partial survive
in its evidence directory (`ledger-round1/staged-report.md`,
`before-ledger-gap-matrix.md`). Disposition: the parent workflow was
stopped, the runner group terminated, the reconciled document restored
from the staged copy, and the runaway output kept beside it as
`ledger-gap-matrix.exhaustive-attempt.md` — an UNREVIEWED
inference-heavy classification that is not authoritative and must not
be cited as coverage. Lesson (binding for this program): a research
lane that returns `unavailable` is finished — never run a fix round on
a document whose producing lane can still resume; the long-timeout
procedure must not resume after an explicit `unavailable`, and the
orchestrator confirms the producing runner is dead before editing its
output. No live state, repo file, or other lane was affected; the
reader and C1 worktree were untouched.

Addendum (05:36 UTC): the runner shell died on the group kill but the
`codex exec resume` child survived (GNU `timeout` places its child in
its own process group); it was terminated separately by verified
command line. The fix lane's FINAL reconciled version (05:30) was
lost — its staged copy (05:24) is an intermediate stage carrying the
retitle, the 51/3,777 coverage with the fifteen promoted codings, the
T8 recode, the intake-design citation, the citation fixes and per-row
labels, but not the spec-delta re-expression, the per-field
disposition or the fix record. Recovery: the remaining directions are
re-run on the staged base through a no-resume variant of the research
procedure (`research-sweep-fix.js`) with the producing runner confirmed
dead first. This is the same round-1 fix, not a second round.

### B — adjudication: reconciled PARTIAL matrix ACCEPTED; increment B stays OPEN

Correction to the incident note: the fix lane's FINAL reconciled
document survived — the runaway turn had renamed it to
`ledger-gap-matrix.turn0.md` before writing its regeneration — and was
restored to the report path after the runaway was terminated. Verified
directly by the orchestrator (2026-09-08 ~05:40 UTC): title "PARTIAL
(increment B open)"; coverage 51/3,777 (36 short-result + 15 hero,
R1/W2 zero) with the continuation stated; W1-0892 recoded as a
contradicted authored `none` with the retained completion limitation;
the accepted intake design added to the authority list and every
SD-* reconciled against SD1–SD6 exactly as ruled (RETENTION-REF = SD3;
SOURCE-TUPLE = study roles + NEW `baseline`, `gated`, `reviewed`,
`red_base`, `red_features`, `landing` covering landed; DIFF-CONFIRM and
BUDGET-AUTHORITY outside the ledger; REVIEW-INTEGRITY = SD4 envelope
with D's enforcement; CLAIM-ASSESSMENT = accepted vocabulary;
NARRATIVE-CUT a renderer requirement); the §4 disposition one row per
field/member with the coded proposition and its class; the unused-field
deliverable recorded as unmet and carried into C's entry conditions;
A2 restated; citations fixed; per-row labels; A1–A15 pointed at this
ledger; appendix intact (3,777 IDs). Promoted: the reviewed part as
`docs/design/ledger-gap-matrix.md`, the full document, work index,
census, fixtures and verification logs under
`docs/design/evidence/ledger-gap-matrix/` (copies of another repo's
ledgers and review files are NOT committed; hashes identify them). The
runaway's regeneration is kept only in scratch as an unreviewed
attempt. The payload-v1 provisional freeze in `ledger-writer-brief.md`
matches this document.

## Lane C1 — ledger writer, fold and standalone verbs (implementer turn complete; orchestrator verification PASS)

`feature-pr` on mesh branch `feat/ledger-writer` (stacked on
`feat/ledger-reader`, worktree `~/projects/mesh-ledger`), implementer
`gpt-6-astra` high, launched 2026-09-08 05:18 UTC; implementer turn
complete at 06:04 with twelve commits (`08fbd55` … `fbfc475`), 3,313
inserted lines (budget 4,000), new dependencies `yaml-rust2` (YAML 1.2)
and `unicode-normalization`, 21 writer tests, gates reported green
(`just check-quick`, `just lint`, `just test` 709 passed / 1 ignored,
the archive check). Opus lenses and gate in progress.

**Orchestrator verification (S, 06:11 UTC, candidate `fbfc4752`, crate
binary in a tempdir root with an empty environment):** a scratch team
with a frozen assignment; `ledger init --wave --authority-file
--repo-root` created `state/ledger/v1/{lock,manifest.json,segments/
000001.jsonl}` and returned ledger/epoch/incarnation ids; `ledger entry
--file result.md` admitted an `outcome` whose body was selected by
`{section: Summary}` — the "Details" section did not enter the event;
limitations with quotes and a pipe survived; the receipt carried
`committed_cut {committed_bytes, last_sequence, prefix_digest}` and a
`request_digest`; `render --view current|narrative` byte-identical
across two runs; an identical retry returned the original receipt at
sequence 1; `amend` with `previous_event_id` moved the head to sequence
2; a stale head returned `head_mismatch`, exit 3, with `current_head`;
`history` exposed the authenticated author envelope (actor,
member incarnation, authority ref) and committed time; a 5,000-byte
body returned `invalid_input`, exit 2, `actual: 5000, limit: 4096`,
"link the complete artifact, never truncate"; `tombstone --json`
landed at sequence 3; a render left the root's file digests unchanged.
Notes for the lens round, not blockers: the Markdown blocks are still
field dumps (`<pre>` JSON) rather than the study's compact contract —
increment D's canonical rendering replaces them; the stale-head detail
string is generic ("current head required; tombstones are terminal").

### C1 — round-1 lenses (fix round r1 running)

Opus conformance lens: **fix_required** (1 major, 4 minors, 4 nits) —
the `--json` intake adapter is the YAML parser behind a JSON pre-check
and rejects valid JSON with surrogate-pair escapes; `cargo deny`
licenses fail because the `yaml-rust2` chain pulls `foldhash` (Zlib)
which `deny.toml` does not allow; admission re-reads the whole workflow
journal once per reference (3 s at the documented limits under the
ledger lock); source-side read failures are reported as
`committed_corruption`; clap's usage is replaced by a ledger JSON error
for any argv containing the token `ledger`; three nits (cfg-stub
convention, `ledger::run` panic-on-misuse, unpinned canonical map
ordering). Opus operational lens: **fix_required** (3 majors, 2 minors,
1 nit) — malformed authored Markdown PANICS (exit 101) instead of a
structured `invalid_input`; Git revision/blob verification resolves
against an enclosing repository rather than the declared `--repo-root`;
`remaining_status: none` is blocked for any referenced completion
unless its whole summary is copied verbatim into `limitations[]`; the
narrative view repeats an identical heading per entry; the scope-owner
authorization rule has no test. The procedure's fix round r1 is
running on `gpt-6-astra` high.

**Orchestrator adjudication on the `remaining_status: none` major
(binding for C1's acceptance):** the implementer's copy-the-statement
rule over-read the C1 brief's T8 fixture (the orchestrator's wording).
The study and the intake design fix the rule: `none` is REJECTED only
against known open obligations that are STRUCTURED (open `remaining`
roots in the same declared scope); it is never rejected on prose; the
writer does not guess which sentence of a referenced completion is a
qualification. The T8 fixture's intent is VISIBILITY, not copying:
when an `outcome` declares `none` and its referenced completion carries
a summary, the projection renders that completion statement beside the
declaration ("declaration: none; completion statement: …") so a
contradiction can never be hidden, and a later obligation flags the
earlier `none` for reconciliation. No copy obligation, no rejection.
The C1 brief's fixture text is corrected to this reading.
