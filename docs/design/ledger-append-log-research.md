# Ledger append-log research

Commissioned Astra researcher/architect study, 2026-09-07. Research only; no implementation or commit. Prepared against the [brief](ledger-append-log-brief.md), for activation after wave 2 closes.

**Recommendation [I]: one small, separately versioned ledger log per team incarnation, with explicit wave/scope references.** It owns authored outcome declarations, remaining-work items, non-task decisions, and annotations linking evidence. Existing tasks, assignments, rulings, completions, and monitor records remain authoritative in their existing stores. A pure reader joins those sources, folds ledger revisions, and generates `ledger.md` on demand. Commit rendered snapshots and their reproducible input bundles at actual boundaries, not after each event. No service, watcher, delivery integration, summarizer seat, or second task database is needed.

The evidence supports eliminating hand-maintained projections, but **does not establish that most full-wave ledger prose already exists in retained structured records**. Four rows contain 85.7% of wave 1's task-table text; the retained structured archive stops before those tasks exist. Treat missing historical retention separately from missing schema. The smallest next increment is an offline reader over the existing archive, followed by an explicit gap-coding review; the bounded census and archive-only rendering trial below already establish its baseline.

## 1. Evidence contract and read-only boundary

Labels follow the [messaging overhaul study](mesh-messaging-overhaul-research.md):

| Label | Meaning in this study |
|---|---|
| **S — source verified** | Inspected source or retained artifact bytes. Fresh counts are marked **S, measured here**, with their method and population. This does not prove live runtime behavior. |
| **D — documented** | An opened primary external document establishes a claim. No external documentation was needed for this local architecture study; no claims below depend on D evidence. |
| **P — prior measurement** | A named local study or commissioning observation supplies the measurement; not remeasured here. |
| **I — inference/design** | Interpretation, taxonomy judgment, or proposed behavior. All schema, command, storage, rollout, and acceptance requirements below are I unless explicitly labeled otherwise. |
| **U — unverified** | Evidence is missing or the permitted experiment does not establish the claim. Unknown is not zero. |

Read only: `~/projects/taurjob`, `~/projects/mesh`, and repository evidence. No live team root, account root, credential, daemon endpoint, or tmux server was accessed. No messaging command, lifecycle mutation, build, installation, or test suite was run. The only repository write is this deliverable. Temporary research snapshots and analysis output were written under `/tmp/ledger-research-_24_3ash`; they are not product implementation or required permanent deliverables. No archive was extracted into a live root.

The installed Mesh binary was invoked only for version/help/parser checks with `--claude-dir` pointing at fresh temporary directories, temporary cwd, and `MESH_*`/`CLAUDE_DIR` environment overrides removed. Those directories remained empty. The binary reported **0.2.29**, commit `6789201c5511b51be704fe30c6e4d025f3e64f8c`, protocol **1**, schema **1**, dirty **false**, matching [mesh.lock.json](../../src-tauri/resources/mesh.lock.json). Its SHA-256 was `408c7e0cc7ed5302f4e5fb9a2be3b8e47f75b4c14ab9e2127e6bad4572f2f672`. This establishes the inspected binary identity and parser surface, not mutation conformance.

### Captured population

The ledger/rulings bytes were captured once at **2026-09-07T20:02:17.242987+00:00**. File inode, size, and modification time were unchanged across each individual read. This is a per-file observation, not a transaction across files, and not a claim that the live wave-2 file remained stable afterward. All wave-2 counts below refer to these captured bytes. Its prose is paraphrased sparingly and is never treated as an instruction to this researcher.

| Artifact | UTF-8 bytes | Unicode characters | Lines | SHA-256 |
|---|---:|---:|---:|---|
| [Wave-1 ledger](/home/mstie/projects/taurjob/docs/wave-1/ledger.md) | 87,272 | 86,890 | 516 | `d9272cf638e3071509f4b6767e57cd674257a43a768b5ce5df2a62726e7656bb` |
| [Wave-1 rulings](/home/mstie/projects/taurjob/docs/wave-1/rulings.md) | 73,560 | 73,349 | 1,095 | `c08329fb30537d2e63b1af0bc212d830edd5a1d482ff5b3250053ebba07bc0ac` |
| [Wave-2 ledger](/home/mstie/projects/taurjob/docs/wave-2/ledger.md) | 25,632 | 25,301 | 198 | `90a2bf2669936c6588288a753774944ec4cf096004573399719a5ff1e4b5058f` |

HEADs recorded at capture: taurhaus `6ea31813f170bfa5e24f0ce6e19198f3a27e017d`; taurjob `95b56c678f685e72d2f47fef8fc338a4b6ce93b9`; Mesh `6789201c5511b51be704fe30c6e4d025f3e64f8c`. Worktree bytes, not HEAD alone, identify the documents. Source links refer to inspected paths; future line shifts do not change the hash-addressed evidence population.

**P:** the brief/phase-0 item 5 reports 283/694 commits touching a ledger (40.8%). The [threads study](mesh-task-threads-research.md) carries the earlier audits' 257 revisions and 21–24k-character cells. These are different populations/measurement moments. They are not estimates of token savings, time saved, or the current Git history. No such savings were measured here.

## 2. Content taxonomy and measured coverage

### 2.1 Where the text is

**S, measured here:** wave 1 has **29 task-table rows**, eight columns, and **52,159 characters of trimmed cell content**. T6a is a document row in addition to the numbered T1–T28 set; it must not silently become a new Mesh task. The table's cell content is 60.0% of the entire ledger. Table parsing here is deliberately specific to these bytes: pipe-separated rows beginning `| T`, with exactly eight cells; no embedded pipe occurs in those data rows.

| Whole-ledger bucket | Characters | Share | Interpretation, not a structured-coverage claim |
|---|---:|---:|---|
| Task identifiers, kinds, owners, acceptance routes, deliverables, commit columns | 3,829 | 4.4% | Mostly fact-shaped; routes and work kinds still require careful adapters. |
| Task `state` and `remaining` cells | 48,330 | 55.6% | Mixed result reports, old/new decisions, review history, measurements, and actual remaining work. |
| Rulings-log section, including trailing retro judgments | 20,614 | 23.7% | Mixed ruling summaries, escalations, corrections, and synthesis. |
| Branch/budget/gate/design/hero/review-route/commit-hygiene sections | 6,378 | 7.3% | Governance and contract facts, often sourced from documents or task descriptions. |
| Lead deliverables, smoke, open escalations | 6,290 | 7.2% | Observations, evidence citations, causal explanations, and dispositions. |
| Preamble, task-section framing, separators, whitespace | 1,449 | 1.7% | Presentation rather than new facts. |
| **Total** | **86,890** | **100% before rounding** | Every character assigned once by section/column. |

The semantic names of those buckets are **I**; their lengths are **S**. In particular, 55.6% is **not** “novel prose,” and 23.7% is **not** “already structured.” Both buckets mix categories.

**S, measured here:** T25–T28 contain **44,706 characters**, **85.7% of task-cell content** and **51.5% of the whole ledger**. T26 alone has 24,180 cell characters, including a 17,433-character `remaining` cell. T28's `state` is 8,520 characters, T25's is 6,653, and T27's is 4,886. The pathology is storing history inside a supposed current-state cell; merely changing Markdown to JSON while retaining those cells would preserve it.

For the captured live wave 2, **S, measured here:** 39 task-board rows, ten columns, 11,590 cell characters. The board section occupies 13,062/25,301 ledger characters (51.6%). `state` contains 5,244 characters; `remaining` 2,164; the longest individual cell is 616. A three-column carried-list row also begins with a task number and is correctly excluded from the ten-column board census. This smaller live board is an observation at the capture time, not evidence of a durable improvement or a forecast of closure size.

### 2.2 Semantic taxonomy: what belongs where

| Content class | Concrete closed-wave evidence | Existing source that can carry it | Genuine gap / rendering rule |
|---|---|---|---|
| Task identity, owner, lifecycle, assignment contract | Ledger review-route says Mesh is canonical; T1–T11 map explicitly | Task objects; workflow creation/state/assignment events; protocol records | Project these. Preserve team incarnation and assignment identity; do not parse `T6a` as task 6 without an explicit mapping. |
| Work kind, acceptance owner, independent-review route | T3 measure, T4 diagnose, T19/T21 heroes | Metadata and assignment descriptions/completion signals; committed wave packet | Historical `docs`, `verification`, `runtime` do not exactly encode the delivery standard's five kinds. A source adapter may expose raw values; a reviewed mapping must label its interpretation. |
| Results and honest limits already authored once | T3 stream measurements, T8 unvalidated platforms | Workflow completion/progress/review summaries and task metadata | These summaries already contain prose. Render/link them; do not require an identical new ledger outcome. “Prose” does not mean “missing from a structured carrier.” |
| Review verdicts/scores and scope | T1 successive rejections; T9 review completion rejects T2 | Task ruling array and workflow `ruling_recorded`; reviewer artifact | Ruling `ref` is not reliably typed as candidate versus report. No last-verdict shortcut; preserve numbered findings, scope, and unresolved questions. |
| Budget ceiling/raise/failure | T11's retained `budget-430`; later T27/T28 ceilings | Task description; ruling `field=budget_raised`; `oversize_diff` | Old/new ceilings and counting semantics are often prose. A ledger annotation may pin a measurement basis, but must reference the actual ruling; it cannot authorize the raise or erase late approval. |
| Monitor/lifecycle observations | Ledger E7, batch-lane idle nudges, board reconciliation | Modern `idle_monitor_records`, workflow events, routing telemetry | Machine observation and lead's explanation are different claims. Do not invent a monitor event from an incident paragraph, or equate a nudge with inactivity. |
| Artifact/candidate/rubric/evidence identity | T26 matrix versus certifying run; T25 rubric changes | Existing `ref`/summary and immutable artifacts | Typed links between candidate, rubric, instrument, scenario/root, completion, and review are incomplete. Supply links/qualification, not copied transcripts. |
| Honest outcome beyond terminal state | T28 delivered 1,108 lines, authored 1,161 including an unlanded 53-line test; T27 probe cause still unconfirmed | Completion summary if explicitly present; otherwise new declaration | Optional outcome annotation with explicit scope, limitations and remaining-status declaration. Task completion does not mean all proposed work shipped. |
| Remaining, deferral, stop and revisit condition | R41 freezes work, then specifically un-defers T28-12; portal helper remains contingent | Some dependencies/rulings; otherwise new remaining item | Name disposition, consequence, owner/route, evidence, revisit trigger and source decision. Never turn `completed` or missing text into `remaining: none`. |
| Wave governance and explanation | E16 shared-checkout incident and successive clarifications; final retro overturns E5 inactivity attribution | Committed policy/ruling/retro artifact; new non-task decision when needed | A bounded decision or note links the authority and what it supersedes. Do not reconstruct causal judgment from timestamps alone. |

### 2.3 What the retained structured records actually reproduce

The [archived team tarball](/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz) and adjacent [task snapshots](/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/tasks) were read directly. No inbox bodies or control-auth files are needed for this census.

**S, measured here:** the archive contains 321 workflow rows, 53 protocol-index rows, and 98 task-mutation rows. There are 11 task snapshots and 23 task rulings. Workflow rows include 11 creations, 12 assignments, one assignment supersession, 72 state updates, seven completions, seven review requests, five progress reports, 23 rulings, and messaging/administrative events. Workflow timestamps span **2026-09-06 19:34:32.013–20:11:26.180 UTC**; this is the retained journal window, not a complete-wave transcript. Adjacent snapshots and the last workflow state agree on status for all 11 tasks; this limited agreement does not establish an atomic archive cut.

There is **one** archived budget-related ruling, task 11 sequence 1 with value `budget-430`, no typed `budget_raised` field. There are **zero** `budget_raised`-field rulings among the 23, and **zero** modern `idle_monitor_records` entries in the 11 task snapshots. The latter schema is newer than this archive. These zeros describe the inspected population, not the full wave.

The following denominators are deliberately separate:

| Coverage question | Result | What it establishes |
|---|---|---|
| How many final table rows have a directly mapped archived task? | **11/29 = 37.9%** | Record availability by row identity; not final-state coverage. T6a and later rows have no independently matching archived task object. |
| How much of a narrow factual surface can be corroborated? | **27/87 = 31.0%** of identifier/sole-owner/single-commit cells across 29 rows | 11 identities, 10 exact sole owners, six matching commit cells. T7's multi-author owner cell is not counted. |
| What is that verified minimum against all task cells/text? | **27/232 = 11.6% of cells**, **219/52,159 = 0.42% of cell characters** | A deliberately conservative lower bound, not an estimate that only 0.42% is reproducible. It excludes semantically matched prose and every untested column. |
| Which compact final accepted candidate results have a matching completion and positive ruling? | **T3, T4, T5, T8: 4/29 = 13.8%** | Candidate/outcome cores are reproducible. Their literal `remaining: none` cells are not independently established by completion. |
| Can the four text-dominant final rows be reconstructed from this archive? | **0/4 source tasks retained** | The archive lacks T25–T28. Their 44,706 characters cannot be counted as structured duplicates or as schema gaps on this evidence. |
| Fraction of all full-wave propositions already captured structurally | **U — not identifiable from retained population** | Do not publish a guessed “80% automatable” figure. Missing later retention and missing typed fields are different causes. |

The narrow factual audit uses literal commit-token occurrence in the matching task's structured record (including its summary/ruling string fields), exact sole-owner equality, and explicit task-ID mapping. It establishes corroboration, not an automatic semantic parser. Corresponding commit hits are T3 `8a7375b`, T4 `c563ff3`, T5 `79d3657`, T7 `d966220`, T8 `bfc2cca`, T10 `b825b54`. T10's candidate/ruling is retained while its task is still `in_progress`; the final ledger's completion must not be backfilled into this cut.

**Judgment [I], high confidence:** ordinary task/assignment facts and already-authored result summaries need no new ledger events. **Judgment [I], medium confidence:** a small annotation log is sufficient because the irreducible material has a few recurring roles: qualified outcome, remaining work, decision, evidence linkage. **U:** the full-wave proportion and expected authoring/token reduction. Low archive coverage is a retention limit, not evidence that a large new schema is necessary.

### 2.4 Bounded archive-only rendering trial performed here

A temporary standard-library Python reader, with no ledger Markdown supplied to generation, rendered all 11 archived tasks, last workflow status, ruling counts, and seven original completion statements with event IDs. Output: **3,435 UTF-8 bytes**, SHA-256 `742ef7d7d1d6fef64f6d520cb8f13068b39674900ec4307bb91b9d2843b597f3`. This is a research projection, not a shipped prototype, complete ledger equivalent, or measured cost saving. The reproducible recipe is in §11.

The four compact accepted cores join as follows:

| Task | Completion workflow event | Ruling on same task | Candidate reference |
|---|---|---|---|
| T3 | `4f28477b-4cba-4c3b-a337-81c7bd886e25` | seq 2, architect, verdict accepted | `8a7375b` |
| T4 | `521117ca-8671-4682-8a79-bf3f04c71cfa` | seq 2, architect, verdict accepted | `c563ff3` |
| T5 | `77faa1e4-7a91-459a-8d2f-7fa35ff4595d` | seq 2, architect, verdict accepted | `79d3657` |
| T8 | `3c5d02db-f5a4-4529-9c05-84778c5ccf20` | seq 2, architect, verdict accepted | `bfc2cca` |

The trial also exposes useful negative cases: T1/T2 are still in progress with correction/rejection history; T6's retained completion candidate differs from the final frozen trio; T7 is a completed review with no qualifying verdict on its own task; T9 completes a review whose verdict belongs on task 2; T10 has acceptance evidence before terminal completion. These are mandatory projection fixtures, not discrepancies to “repair” by reading the final ledger back into the input.

### 2.5 Deep cases that determine the schema

1. **R36/R38: revision of interpretation versus new observation.** The rulings document has 32 second-level sections, not 41 independent records. R36a and R38 contain amendments internally; R36b/R37a shares a heading. The retained 56→82→56 history changes population/instrument coverage. A repeated numeric value is not the same observation. Record instrument, candidate, population and evidence separately. Correcting a transcription supersedes a claim; measuring a different instrument appends a new observation linked to the previous one. Never tombstone the earlier FAIL because later certification passes.
2. **T25: operative rubric versus amendment chronology.** Its state cell carries 6,653 characters of amendments and stale “tip at this writing” pins. The operative rubric must be an immutable artifact reference for a specific review/measurement, with history reached through explicit links. Rendering `HEAD` at read time cannot reproduce the rubric used at RESULT time.
3. **T26: several meanings of PASS.** The matrix run certifies pre-fix code, later reruns certify different rows, and independent reviewers distinguish checked-from-artifacts, accepted-on-citation and unmeasured claims. One boolean outcome loses the claim. The renderer needs an exact evidence scope and qualifications, and must retain per-root zeros and dashes in the canonical result artifact.
4. **T27/T28: task terminal state versus scope fulfillment.** Batch lanes close with delivered work, unlanded authored work and explicit deferrals. Keep source/landing commits distinct, include the counting basis, and preserve unresolved cause statements. An outcome declaration is not permission to complete a task.
5. **R41/E5: authority and correction.** A freeze is amended by a specifically authorized exception; a retro later overturns a seat-inactivity explanation. Neither should be implemented as “latest paragraph wins.” The event names exactly what it replaces and why. Historical original authorship and the operator/lead's authority remain inspectable.
6. **Captured wave 2:** the board separates scope/reviewer/budget but still repeats budget raises in board and ruling log, and carries pending evidence in rows marked landed. This supports independent lifecycle, acceptance, delivery and remaining fields. It supplies no license to modify or migrate the running wave.

## 3. Existing substrate and authority boundaries

**S:** these are the inspected implementation seams, not promised behavior of a deployed process.

| Source | Existing contract | Consequence |
|---|---|---|
| [workflow.rs](/home/mstie/projects/mesh/src/workflow.rs:39) | Event ID, protocol version, typed creation/assignment/progress/completion/ruling bodies; append under lock, `sync_all` | Reuse source IDs and summaries. It lacks a global logical sequence/commit manifest. Do not add ledger prose variants to this existing enum. |
| [protocol_index.rs](/home/mstie/projects/mesh/src/protocol_index.rs:20) | Assignment/command facts; append-upsert, latest physical row per `recordId` | Current protocol rows are a projection source. Historical reads need the selected prefix before latest-by-ID folding. No ordinary body archive is present. |
| [task_journal.rs](/home/mstie/projects/mesh/src/task_journal.rs:13) | Task ID, actor, timestamp, changed-field **names**, byte-offset API | It is a mutation/correlation journal, not a before/after value log. It cannot reconstruct arbitrary historical metadata. |
| [tasks.rs](/home/mstie/projects/mesh/src/tasks.rs:187) | Task mutation validates under task-directory lock; task write and journal append are distinct operations | A missing workflow/journal echo is possible; select authority by field, expose discrepancies, never count mirrors as separate acts. |
| [task_lifecycle.rs](/home/mstie/projects/mesh/src/task_lifecycle.rs:384) | Ruling kinds `verdict`, `score`, `ruling`, `note`; per-task sequence; by/at/ref/note; scalar verdict/score mirrors | Ruling identity is `(team incarnation, task ID, ruling seq)`. R1 document heading is not task ruling seq 1. Scalar latest verdict loses candidate/scope history. |
| [main.rs ruling handler](/home/mstie/projects/mesh/src/main.rs:4185) | Authenticates an active member, mutates task ruling, then records workflow ruling | Current API does not enforce the full delivery standard's acceptance-owner policy. A future ledger writer must explicitly validate its narrower authority; do not imply that today's arbitrary ruling values carry stronger enforcement. |
| [idle_monitor.rs](/home/mstie/projects/mesh/src/idle_monitor.rs:618) | Source-tagged monitor records in task metadata, mutation journal notification | Use source tag/message identity; keep deadline/manual sources separate. Never create a competing activity model. |
| [delivery standard](../team-delivery-standard.md) | Five-line assignment, work-kind evidence, frozen review contract, honest limits, budget chronology, result/reviewer artifacts | Obligations live at canonical artifacts. Rendering may link them, not weaken them or impose a new repeated result ceremony. |

Workflow and task-mutation offset readers currently read whole files, then filter by byte position; an offset beyond length rewinds; malformed rows are warned about and skipped. They can parse an unterminated final JSON row. Neither behavior is an acceptable committed-prefix contract for the new ledger authority. Protocol index also replays its rows. Reuse concepts and validation helpers, not the current recovery semantics wholesale. [Workflow reader](/home/mstie/projects/mesh/src/workflow.rs:1191), [mutation reader](/home/mstie/projects/mesh/src/task_journal.rs:62).

### Storage choice

| Alternative | Decision |
|---|---|
| Append new variants to workflow/task-mutation/protocol journals | Reject: changes existing reader contracts and blurs ownership; task mutation records do not even contain values. Violates the overhaul study's separate-authority rule. |
| Rows in the future message journal | Reject: ledger has no recipients, unread state, delivery, wakeup, private-message retention or transport dependency. Its adoption must not wait for messaging migration. A message may cite a ledger event later without owning it. |
| One log per task or Markdown file per event in Git | Reject for v1: duplicates cross-task decisions, complicates wave ordering/closure, and retains per-event commit ceremony. |
| New SQLite ledger/task mirror | Reject: unnecessary second database and lifecycle drift risk for this small append workload. |
| **Separate team-incarnation ledger journal** | **Choose:** one local sequence, one writer contract, cheap full replay, scope/task links, independent versioning. A wave can reference more than one incarnation explicitly. |

Proposed layout under an explicitly resolved team root: `state/ledger/v1/{lock,manifest.json,segments/000001.jsonl}`. Runtime events are outside the project Git worktree. A logical ledger ID and team-incarnation ID are required; a reused team name or alternate account root is not identity. Resolve team authority with the existing explicit root machinery; never scan roots and take the first matching name. Wave and project IDs are explicit references, not inferred from cwd.

## 4. Minimal event contract

### Envelope and operations

This is a **proposed ledger schema version 1**, independent of Mesh protocol/schema 1 and taurhaus app/daemon protocol 24. It assigns no future Mesh release number.

| Field | Contract |
|---|---|
| `schema_version`, `ledger_id`, `team_incarnation_id`, `log_epoch` | Fixed authority/format identity. Epoch changes only for an explicit storage reset/repack, not each process restart. |
| `event_id` | Client-generated UUID, also the retry key, unique within the ledger. Same ID and canonical request returns the original receipt; changed request conflicts. |
| `sequence` | Writer-allocated positive integer, contiguous within the committed epoch; never chosen by the client. |
| `committed_at`, optional `occurred_at` | Writer admission timestamp and producer-described observation time. Sequence orders; neither clock resolves conflicts. |
| `author` | Authenticated actor ID, member incarnation, and authority/policy reference. Original claim author and revision author remain distinct. No arbitrary `--author` impersonation. |
| `operation` | Exactly `entry`, `amend`, or `tombstone`. |
| `entry_id`, `entry_key` | Root identity and immutable logical key. For `entry`, root ID equals event ID. Key = wave + primary scope + kind + caller-declared slot. |
| `previous_event_id`, `reason` | Mandatory for amend/tombstone; names the current head of that root. Nonempty specific reason required. |
| `references` | Typed immutable task/scope/assignment/artifact/ruling/completion references; multi-task links allowed, one primary scope. |
| `payload` | Full replacement claim on entry/amend; no field-level merge patches. Tombstone has no replacement claim. |

A task reference includes its team incarnation and task ID, plus assignment ID whenever the claim concerns a particular delivery. A scope reference includes project, wave, scope ID, and packet revision. A source-event reference names authority, source identity and event ID; a legacy row without an ID names captured source digest and exact byte interval. Ruling references include task and sequence, not just a bare `#2` or `R38`. Artifact references carry repository identity, immutable revision/blob/content digest, path and optional heading/line locator. Local probe artifacts use a digest and retained bundle locator, not a promise that `/tmp` will survive.

Use typed reference roles such as `candidate`, `landing`, `rubric`, `instrument`, `result_artifact`, `review_artifact`, `completion`, `ruling`, `prior_observation`. A reference's existence and its semantic claim are separate: resolving a file does not prove it supports a PASS. Require an evidence assessment (`source_checked`, `accepted_on_citation`, `unmeasured`, `unavailable`) and a limitation when applicable. These are product evidence fields; the memo's S/D/P/I/U labels remain research provenance and do not magically confer authority.

### Four payload kinds, no duplicate status store

| Kind | Minimal fields beyond references | Authoring rule |
|---|---|---|
| `outcome` | Bounded `body`; declared scope disposition (`fulfilled`, `partial`, `stopped`, `unknown`); `limitations[]`; `remaining_status` (`none`, `items`, `unknown`) and remaining-entry refs when `items` | Optional qualification/declaration only when existing completion/result evidence does not already say it. Scope owner declares scope fulfillment; task owner may report their own delivery. No `task_status` or lifecycle verb. |
| `remaining` | Stable item identity; description; disposition (`open`, `routed`, `deferred`, `resolved`, `withdrawn`); consequence; optional owner/target task; revisit/release condition; resolution/decision evidence when closed/deferred | One meaningful obligation per entry. A route is not completion; a deferral must say when/why to revisit. `resolved` requires evidence and authorized disposition. |
| `decision` | Question, decision, consequence, authority reference; applicability/scope; optional prior decision link | Wave governance within the actor's mandate. Task assignment, budget, review acceptance, GO, or lifecycle effects still require their existing commands/rulings first. A ledger decision only records/cites those effects. |
| `note` | Bounded body and evidence/relationship references; optional `qualifies` target | Measurements, corrections of explanations, and missing typed joins. No implication of acceptance, task progress, or read acknowledgment. |

Do not add first-class task creation, assignment, completion, verdict, score, budget raise or monitor event kinds. A result artifact already containing method, limits, red evidence and per-root counts is linked once. If the completion summary suffices, the ledger author does nothing. A note can supply a typed completion-to-review association without copying either body.

Proposed practical limits: 4 KiB body, 32 KiB total serialized record, 64 references. These are initial design bounds, not measured optima. Reject oversize input with an actionable link-to-artifact instruction; never silently truncate a qualification. Large findings/score tables remain in their canonical artifact. JSON escapes newlines so each event occupies one physical JSONL line. Control characters and unsafe Markdown/URL rendering are rejected or escaped; no shell execution occurs while resolving a citation.

Example shape, abbreviated IDs for readability and **not an executable fixture**:

```json
{
  "schema_version": 1,
  "ledger_id": "ledger-uuid",
  "team_incarnation_id": "team-uuid",
  "log_epoch": "epoch-uuid",
  "event_id": "event-uuid",
  "sequence": 42,
  "committed_at": "2026-09-08T10:00:00Z",
  "author": {"actor_id": "owner", "member_incarnation_id": "member-uuid", "authority_ref": "policy-revision"},
  "operation": "entry",
  "entry_id": "event-uuid",
  "entry_key": {"wave": "wave-3", "scope": "S1", "kind": "outcome", "slot": "delivery-A"},
  "references": [
    {"authority": "workflow", "event_id": "completion-uuid", "role": "completion"},
    {"authority": "artifact", "repo_id": "repo-uuid", "revision": "full-commit-id", "path": "results/S1.md", "role": "result_artifact"}
  ],
  "payload": {
    "body": "Delivered the bounded slice; the unlanded experiment remains deferred.",
    "scope_disposition": "partial",
    "limitations": ["No production run measured."],
    "remaining_status": "items",
    "remaining_entry_ids": ["remaining-uuid"]
  }
}
```

Production validation requires complete source identity in every reference, real UUIDs/digests, actual task/assignment and scope mappings, and a resolved authority reference. The example is intentionally not a new competing assignment/result template.

### Authorship and authority

All writers use the authenticated Mesh actor path and validate permissions against an immutable policy/assignment reference. For v1, use a frozen wave acceptance/ownership manifest generated from the approved packet; changes require an explicitly authorized manifest revision. This manifest is authority metadata, not another task board. Source references identify the policy version used at admission, so replay does not depend on today's roster.

The active policy digest belongs in the ledger manifest. Initialization fixes the genesis policy at sequence zero. Later policy activation uses an explicit lead-authorized `decision` entry, with a reserved authority-policy scope/slot and typed `authority_change: {previous_digest, next_digest}`; the referenced immutable policy document supplies the new permissions. Validate activation under the old policy, retain both documents, and publish the event plus new active digest in the same manifest transaction. This is ledger-local governance, not a fourth operation or task mutation. The fold replays these decisions; policy activation is not retrospectively amendable, and another policy change requires another activation entry. Requests name the policy digest they were prepared against; a changed digest fails admission for revalidation. Do not validate a mutable policy outside the lock and then append against a different policy. Actor authentication still follows the existing Mesh boundary; the ledger does not mint credentials or manage team membership.

| Action | Who may do it |
|---|---|
| Add an ordinary note or report own delivery | Active authorized member within assigned/readable scope. Non-authoritative observations occupy author-specific slots. |
| Amend/tombstone own note | Original author, with current-head precondition and reason. Reassignment does not transfer authorship. |
| Declare a scope fulfilled; dispose of another owner's remaining item | Named scope acceptance owner, or explicitly authorized lead/operator delegation. |
| Amend another actor's ledger declaration | Named acceptance owner/lead with explicit override reason and authority reference; record revision author, retain original attribution and bytes. Ordinary members cannot do so. |
| Alter task lifecycle, reviewer verdict, budget authority, assignment or GO | **Never through the ledger.** Existing commands and source authority remain required. Even the lead cannot use a ledger override to rewrite a reviewer's source ruling. |

For an inactive original author, a currently authorized lead can record a correction under their own identity. Independent reviewers' unreleased evidence is not exposed by adding a task reference. V1 ledger is team-visible material only; private deliberation stays in its existing authority. A publication links a permitted result, not private message bodies. Filesystem-shared actor authentication is a cooperative integrity boundary, not tamper-proof access control against arbitrary file editing.

## 5. Deterministic fold, conflict prevention, and replay

### Admission eliminates competing heads

Under the single stable ledger lock, the writer recovers the committed tail, checks identity/authority/schema, looks up the retry key, validates references and the root head, then allocates a sequence and commits.

* `entry` requires an unused event ID and unused `entry_key`. Root IDs and keys never change. A second creator of the same canonical slot gets `entry_exists`, even if its UUID differs. Independent observations use different explicit observation/author slots.
* `amend` requires `previous_event_id == current_head`, same root/key/scope/kind, and a complete validated replacement payload. A stale edit gets `head_mismatch` and the current head; it is not merged or automatically retried with changed content.
* `tombstone` has the same head and authority checks. It retires the operative claim, leaves a visible withdrawn stub/reason, and never deletes the source evidence. It cannot resolve linked obligations or erase a budget violation.
* Tombstones are terminal in v1. Their keys remain reserved. Reinstatement is a new explicitly named declaration slot linked as replacing the withdrawn declaration, authorized by the same acceptance policy; it cannot silently reactivate an old root.
* An identical retry returns the original receipt **before** applying stale-head validation, provided the authenticated actor is entitled to the receipt. Otherwise a lost response would turn a successful amend into a false conflict. Changed payload under the same ID always fails.

Two concurrent amendments of the same root therefore cannot both be accepted. Amend/tombstone and duplicate-create races have the same single-winner rule. This prevents storage update conflicts by construction. **It cannot make contradictory human observations impossible.** Such observations remain separate evidence; an operative declaration requires a designated authoritative slot and explicit supersession. Never claim a CRDT or “last writer wins” solves disagreement over whether a scope is fulfilled.

### Pure fold

```text
fold(committed_prefix, source_cut, renderer_version):
    verify epoch, schema, consecutive sequence, record identities and integrity
    roots = empty; keys = empty
    for event in ascending sequence through requested ledger sequence:
        verify admitted authority proof/reference and operation invariants
        if entry: create root with current head/payload and reserve key
        if amend: require named previous head; replace payload; preserve ancestry
        if tombstone: require named previous head; mark root withdrawn
    join source facts from the exact input cut, deduplicated by source identity
    emit current roots plus independent task/assignment/ruling facts and coverage
```

The fold never writes a task, sends a notice, clears a wait, changes a read mark, or rewrites an input artifact. Invalid committed data stops the authoritative fold at the verified boundary and reports an error; it is not silently skipped. A diagnostic partial view must be labeled incomplete and cannot be a certifying snapshot. Unknown event/schema versions fail closed for an authoritative render.

`remaining_status: none` is an attributed declaration at a cut, never an instruction to delete remaining items. Admission rejects it when known open items in the same declared scope already contradict it. If a new obligation or source finding arrives later, the current projection shows that item and flags the older declaration as needing reconciliation; it does not keep showing an unqualified “none,” nor silently amend the author's event. Resolved/withdrawn remaining items require their own explicit revision and evidence.

### Time and source cuts

`--at-sequence N` means the ledger's knowledge after event N, not “the whole team at wall-clock T.” `occurred_at` may be older than the preceding event. An amendment arriving tomorrow does not retroactively alter yesterday's view. A historical-corrected interpretation is a separate explicit query/view, not a change to the original replay result.

There is no global sequence across independent authorities. A reproducible projection input is a **vector cut** containing:

* ledger ID/epoch/last sequence and committed byte boundary;
* each workflow/protocol/mutation source's identity, complete prefix boundary and digest;
* the exact task snapshot bytes/digests and their capture interval;
* referenced packet/authority policy/artifact revisions, adapter and renderer versions;
* source availability, known gaps and capture consistency status.

Normal pull rendering captures per-source stable bytes and says `consistency: vector_cut`; it must not imply a cross-file transaction. Retry boundedly on detected replacement/change, then show unavailable/inconsistent rather than spin. Legacy source files are read without invoking Mesh commands that may implicitly touch activity or projections. Source replacement requires identity/digest validation, not length alone.

At closure, the lead freezes the wave's authorized writers and captures the sources after operations settle. That is a workflow boundary, not a new daemon. A cut that references a missing required ruling is incomplete; recapture after the existing authority commits, or explicitly close with that evidence missing. Since the mutation journal lacks values, arbitrary historical task metadata cannot be replayed unless the required old snapshots were preserved. Never substitute current task JSON into a historical render.

Ordering within a journal is source sequence/position. Cross-source display groups facts by scope/task and uses a stable tuple of source rank and position, with explicit causal links; it does not pretend wall-clock sorting provides causality. Original event time, source position and consumer first-observed time remain distinct.

## 6. Storage, durability and recovery without a service

Use one shared validated writer library called synchronously by CLI commands. POSIX/WSL is the initial writer owner, matching Mesh's existing native ownership direction. A Windows Scope reader can consume exported data; it does not append through UNC. Native cross-platform writer support requires filesystem/locking conformance, not an assumed equivalence.

The manifest names schema/ledger/epoch, ordered segment IDs, sealed-segment digests, and the committed active-segment byte/sequence boundary **and prefix digest**. Validate the prefix digest as well as JSON/sequence structure so valid-looking changed bytes cannot pass unnoticed. This detects accidental corruption, not malicious replacement of both data and manifest. A stable `lock` file is never renamed/unlinked. Caches of root heads and retry keys are rebuildable; at this size a full replay is acceptable until measured otherwise.

Commit sequence under lock:

1. Validate manifest and committed prefix; handle only an uncommitted suffix under the recovery rule below.
2. Validate request against folded head and authority; encode one canonical request and event. Idempotency hashes exclude writer-assigned sequence/time but include author, operation, references and payload.
3. Append the complete JSON line plus newline; sync the segment. New segment creation also requires directory-entry durability.
4. Write a new manifest to a sibling temporary file, sync it, atomically replace the manifest and sync its containing directory.
5. Return event ID, sequence, root/head, request digest and committed-cut identity. Success means durable admission, not successful future rendering.

The linearization point is manifest publication; a durable success receipt is returned only after the required syncs. Lockless readers read one complete manifest and immutable segment prefixes no farther than its boundary. A writer may append after that boundary without changing the reader's prefix.

There is a narrow distinction between visibility and acknowledged durability: a lockless reader can observe the renamed manifest before the writer finishes the directory sync. Such a read is a visible cut, not proof that the writer received a durable receipt. A certifying boundary export briefly acquires the same lock, validates and syncs the captured manifest/segments/directory, then releases it and reads the immutable prefixes. Normal rendering remains read-only and does not perform that durability barrier. Test this window explicitly; do not promise that every lockless observation survives immediate power loss.

| Failure | Required behavior |
|---|---|
| Crash before append or during line write | Manifest still exposes the old prefix. On the next write, preserve/quarantine the uncommitted suffix for diagnosis, remove it from the active tail under lock, and retry normally. |
| Full line synced, manifest not published | Still uncommitted. Same suffix rule; do not promote it merely because it parses. Its sequence may be reused because it never entered committed history. |
| Manifest published, caller lost response | Recovery validates the advertised prefix. Same event-ID retry returns original receipt; no duplicate event. |
| Crash around directory/manifest sync | On restart validate whichever manifest survived. No acknowledged event may disappear on a supported tested filesystem. Unsupported durability semantics are an activation blocker. |
| Malformed or missing bytes **inside** committed prefix | Stop with `committed_corruption`; no silent skip/truncate/rewind. Restore a verified backup or perform an explicit repair with a new epoch and coverage record. |
| Reader sees truncated source or same-size replacement | Reject stale source identity/cursor and request bounded resynchronization; never trust byte length as identity. |
| Disk full / failed sync | Return failure or unknown admission as appropriate; retry with same event ID. Do not claim success from a cache. |

Only never-committed suffix bytes may be truncated during automatic recovery. Committed event bytes are immutable. A read performs no repair. If recovery could invalidate advertised bytes, it is not routine recovery and requires an explicit repair process outside this v1 happy path.

V1 may operate with a single segment for the entire wave. Keep the manifest segment-aware from the start, but defer active size-based rotation until workload warrants it. Rotation, when added, seals/syncs the current segment, creates/syncs the next and publishes under the same lock; no reader-visible segment is replaced. No age-based deletion is justified for this small wave record. At boundary, archive the whole committed log and required source cuts. “Compaction” initially means regenerating caches/checkpoints, not deleting old observations or revision chains. A checkpoint includes its input prefix digest and fold version and is always dispensable.

No merge of independently forked writable ledger files is supported. Exported logs are read-only replicas. Moving the writer requires an explicit handoff and old-writer exclusion; switching account roots must not create two authorities for the same ledger. If a fork is detected, report it and stop authoritative writes; never sort two locally allocated sequences by time.

## 7. Pull rendering and the command contract

**S:** locked Mesh 0.2.29 supports `task ruling` and the documented identity/root flags. The scratch parser probe `mesh --claude-dir <temporary-root> ledger --help` exited **2: unrecognized subcommand `ledger`**. `--help`, `--version`, `version` and `task ruling --help` exited 0. There is no shipped ledger command to cite as executable today.

The following is a **future parser specification [I], not current role text or instructions to run during wave 2**:

```text
mesh ledger init --wave <wave> --authority-file <policy.json> --team <team> --name <lead> --claude-dir <root>
mesh ledger entry --file <entry.json> --team <team> --name <author> --claude-dir <root>
mesh ledger amend <entry-id> --expect-head <event-id> --reason <reason> --file <replacement.json> --team <team> --name <author> --claude-dir <root>
mesh ledger tombstone <entry-id> --expect-head <event-id> --reason <reason> --event-id <uuid> --team <team> --name <author> --claude-dir <root>
mesh ledger render --wave <wave> --format markdown --team <team> --name <reader> --claude-dir <root>
mesh ledger render --input-bundle <bundle> --at-sequence <n> --format json
mesh ledger history <entry-id> --after-sequence <n> --limit <n> --team <team> --name <reader> --claude-dir <root>
mesh ledger snapshot --wave <wave> --boundary <boundary-id> --output-dir <dir> --team <team> --name <lead> --claude-dir <root>
```

Files supply exact Unicode/newlines and client event IDs. Amend files contain the new event ID and full replacement payload; envelope fields controlled by CLI/writer cannot be overridden. Offline bundle reads require no active member identity and cannot fall back to a live root. Normal reads require explicit root/team/reader, so examples work without assumed pane environment variables. `init` is explicit and refuses an existing authority; reads never create/migrate a ledger. `snapshot` writes only an explicitly requested export directory, not task state, and does not run Git.

Return structured success/error JSON for mutations and optional `--format json` for read surfaces. Specify and test distinct errors: `unsupported_schema`, `wrong_authority`, `unauthorized`, `entry_exists`, `head_mismatch`, `idempotency_conflict`, `missing_reference`, `source_incomplete`, `cursor_expired`, `committed_corruption`, `durability_unknown`. Proposed exit classes: 0 success, 2 argument/schema error, 3 precondition conflict, 4 authority error, 5 source/storage failure. These numeric classes are future CLI decisions, not current Mesh behavior.

**Execution-validation gate:** before any verb enters role YAML, implement its parser and handler, substitute fixture IDs/files/root/identities in every example, and execute against the newly built candidate binary in isolated tempdirs. Test successful entry→amend→render→history→tombstone, deliberate failures, and snapshot replay. `--help` or token-presence checks alone are insufficient. Then update roles and pin through the normal post-wave release route. [Existing identity-example regression test](../../src-tauri/src/templates/types.rs:1586) explains why relying on `MESH_TEAM`/`MESH_NAME` in copied role commands is unsafe. No such role changes are made here.

### Markdown contract

The default rendering is a compact current projection, not a history concatenation:

* Generated banner: wave/team incarnation, boundary or source-cut ID, ledger sequence, adapter/renderer version, source coverage and consistency. No invocation-time `now` value: the same cut produces byte-identical output.
* Stable task order from explicit scope order then natural task ID. One short table row per task: task, scope, owner, lifecycle, acceptance evidence, operative result reference, remaining summary. Show `unavailable` rather than guessed data. Keep unplaced tasks and unresolved refs visible.
* One anchored block per active outcome/decision/remaining item, with author and evidence links. The table links to these blocks rather than placing paragraphs into cells.
* Separate source authority from declaration: for example, `task: completed; acceptance evidence: partial; scope declaration: partial; remaining: 1 deferred item` is a valid state.
* A history count/link for superseded/withdrawn entries. An unresolved rejection cannot disappear behind a fold; show it in the current acceptance summary. Repeated unchanged policy is linked once.
* Fixed wrapping, deterministic escaping/order, no auto-width tables, no wall-clock elapsed fields, no full repeated gate transcripts. No silent truncation: a long source summary gets an explicit bounded excerpt and canonical link; required qualifications remain visible alongside the outcome.

Illustrative generated block for a historical fact pattern, **not claimed to come from the early archive**:

```markdown
### T28 · closure lane

Lifecycle: completed [completion source].
Declared delivery: 1,108/1,200 lines; 1,161 authored including 53 unlanded.
Candidate: 115e1e6 [immutable result artifact].
Acceptance: both required families [exact review refs].
Remaining: T28-11 deferred by R41; revisit in the next wave [remaining item].
Limit: the R37 guarantee applies from the fixing build forward.
History: 11 delivered batches; earlier candidates and the stopped test [history].
```

A full result/review view must preserve the delivery standard's commit-or-none field, method and instrument limits for measure/diagnose, red-first evidence or explicit justified skip, exact candidate/rubric and source/landing distinction, numbered findings, questions separated from defects, and unaltered score values. These need not all be duplicated in the table. The canonical artifact remains expandable/exportable; an unavailable artifact is an explicit closure gap. Do not invent a score table when none is required by the work kind/assignment.

### Snapshots and wave closure

Render to stdout by default. Writing `ledger.md` is an explicit snapshot operation with atomic output replacement; manual edits carry no authority and are overwritten only at the caller's requested path. Snapshot policy: wave open, actual accepted milestone freeze, wave close, and an explicitly requested audit cut. No snapshot commit for routine progress, amendment, polling, every review, or every render.

A committed boundary deliverable consists of `ledger.md`, a cut manifest, and a compact input bundle containing the required immutable journal prefixes/task snapshots and non-Git evidence or retained artifact references. Secrets/private bodies are excluded; the ledger is a public team record. References alone to a disappearing runtime root are insufficient. The manifest records content hashes and replay versions, excludes ephemeral absolute paths from rendered output, and permits offline reconstruction. It may link existing Git-addressed artifacts instead of duplicating them, provided their repository/object retention is assured.

Do not blindly copy an entire workflow/task source into a public bundle: external-message events or task descriptions can contain material outside the permitted ledger audience. Where raw source export is unsuitable, retain an explicit, immutable derived `source_facts` export containing the permitted fields required by the projection, original source IDs/positions/digests, adapter version, and omission/coverage markers. Replay is then exact for that declared derived input, not a claim to reproduce the entire original journal. If an omitted fact is required to substantiate closure, export authorized evidence through the proper route or mark the closure gap; never publish a private body merely to make replay convenient.

The [wave-closure report](../../../taurjob/docs/wave-1/wave-closure-report.md) pattern consumes the **same structured projection/cut**, not Markdown scraped from the generated table: closed slices; lifecycle/acceptance differences; candidate/rubric/instrument by evidence family; remaining/deferred/stopped items and triggers; budget compliance; scope freezes/exceptions; unmeasured claims; and demo/result artifact references. A lead can still author synthesis and demo instructions. That bounded interpretation is signed/attributed and links the cut, rather than copying event history. Boundary identity is known before the snapshot commit; do not embed “this commit” as a self-referential evidence hash.

## 8. Consumers and acceptance joins

### Routing report and telemetry

**S:** [routing_report.rs](../../src-tauri/src/coordination/routing_report.rs:277) marks accepted when `accepted_eligible && has_review_ruling`; eligibility is task `Completed`. [The scanner predicate](../../src-tauri/src/task_scanner/claude.rs:155) accepts the presence of a `verdict`, `score`, or `ruling` kind, excluding budget raises and oversize-diff failures. It does **not** require a positive verdict value. `note` is excluded. Preserve this existing metric's meaning and historical denominators; document it as completion with qualifying ruling evidence. A new ledger declaration must not increment it.

Ledger links can sharpen a **separately named** future `accepted_candidate`/acceptance-coverage view:

1. Start with an authoritative task completion event and its assignment/delivery identity. A later task reopening invalidates “currently completed” but does not erase the historical delivery.
2. Identify exact candidate, rubric and required reviewer lenses from the approved assignment/review manifest.
3. Join source rulings/review artifacts by typed task/delivery/candidate references. A ledger association may supply a missing link only when recorded by the acceptance owner with evidence; it remains an attributed association, not a newly fabricated ruling.
4. Require positive acceptance from the named authority for each required lens, with explicit resolution/supersession of blocking rejections on that scope. One latest scalar verdict, a budget approval, or completion of the *review task* cannot satisfy it.
5. Separate `accepted`, `accepted_with_recorded_residuals`, `rejected`, `incomplete`, `unresolved` at the acceptance-coverage layer. Unknown or ambiguous joins remain out of the accepted-candidate numerator and appear in a coverage count.

Deduplicate a ruling's task-array copy and workflow echo by `(incarnation, task_id, seq)`; disagreements on those bytes are a source conflict. Do not count the note linking it as another review. Candidate source and landing hashes may differ; require an explicit landing/equivalence artifact rather than an equality guess. Ruling-before-completion is valid (T10 demonstrates this); clocks do not decide authority. Reassignment does not discard prior candidate-specific review evidence, but current scope/rubric must still match.

Keep budget raises and oversize failures as source-authority events, attributed to the owner/assignment at the event. A post-hoc raise remains late. Existing prose budget notes permit displayed evidence, not reliable arithmetic without a typed counting basis. If future budget arithmetic is required, extend the **ruling/budget authority** deliberately; do not install numeric budget authority in the ledger. Reviewer tokens/duration still require launch/task attribution at collection time; no ledger can recover missing cost data from verdict counts.

### Scope walk and history

The [Scope synthesis](team-comprehension-view/synthesis.md) and [design proposal](team-comprehension-view/design-proposal.md:141) require template-generated bounded lines, explicit folds, first-observed read marks and declarations distinguished from observed task motion. Feed Scope the same projection JSON, source IDs and coverage. Scope may request it when opened/refreshed; this work adds no watcher or scheduled wake.

Use source ID/epoch/sequence plus Scope's retained first-observed position. An old-timestamp correction arriving after a read mark must still appear in the walk. Scope read marks never acknowledge a ruling, resolve a dispute, mark an unseen folded item read, or update lifecycle. Pagination cursors advance only over returned evidence; the UI explicitly decides what was seen. A “filed” scope requires an authorized explicit scope outcome, not all child tasks completed. Remaining items and unresolved evidence remain reachable even when the history is folded.

### Retrospectives

Retro tooling consumes boundary cuts and revision history: how many distinct declarations were corrected, how many source facts were merely rendered, review rounds and authorized depth decisions, late budget approvals, explicit stops/revisit triggers, evidence gaps, and stale projection contradictions. Count logical events, not Markdown edits or multiple mirrors. Analyze authored bytes separately from generated bytes and message/token usage separately from ledger bytes. The archive establishes examples and output size, not a causal claim about bill reduction.

## 9. Migration and smallest implementation increments

**Wave 2 finishes unchanged on the hand ledger and Mesh 0.2.29.** No migration-on-read, backfill, daemon action, updated role command, binary pin change, or dual-writing experiment during the live wave. This study changes only this Markdown deliverable.

**Historical waves remain documents.** Preserve wave-1/wave-2 ledger/ruling/result artifacts as immutable historical snapshots and link them from the wave-3 baseline. Do not convert “R41 paragraph 5” into an invented historically authenticated event. An optional historical import later is a separate read-only derived corpus with source digest/line range, import time, original claimed author and explicit `imported_unverified` provenance. It must never be admitted as native event history, used to change metrics silently, or mixed into the new writer's sequence as though it happened at the original time.

| Increment, after the live wave | Deliverable | Advance/stop condition |
|---|---|---|
| **A. Offline existing-record reader** | Read-only adapters plus cut manifest; generated early wave-1 board/result cards; census and source-gap report | Reproduce §2 counts and negative cases; original inputs unchanged. No writer or message dependency. Extend the research-only trial into tested production-quality reader code. |
| **B. Gap specification** | A reviewed proposition-to-source matrix for retained records and selected late result/ruling artifacts | Every gap labeled retention, source-adapter/link, existing-authority schema, or new authored declaration. Do not add ledger fields to compensate for missing archived tasks. Freeze payload v1 only after this review. |
| **C. Isolated ledger writer/fold** | New log envelope, CAS/idempotency, authority checks, manifest durability, pure render; synthetic crash/race fixtures | All §10 invariants pass; no existing journal enum/task store changes. Operates only in test roots until activated. |
| **D. Boundary exports and role conformance** | Offline replayable snapshot, CLI execution tests, canonical compact Markdown | Byte-identical replay; closure obligations reachable; commands executed against actual candidate binary with fixture identity. Only then prepare new role examples. |
| **E. Wave-3 adoption** | Explicit new team-incarnation ledger, approved authority/scope map, handover baseline links to closed waves | Release/pin review after wave-2 closure. Ledger is pull-only; task results recorded once in existing completion paths. Lead snapshots only at boundaries. |

The complete next reader experiment should generate as much of a wave-1-ledger equivalent as retained records support, not copy the hand ledger into “generated” output. Compare by **propositions with provenance**, not Markdown byte equality. Report a row/field denominator, Unicode-character presentation denominator, and proposition denominator separately. Stratify short measure results, hero implementation/review, T25 rubric, T26 integration, T27/T28 closure, and governance. Have the acceptance owner check ambiguous coding. The existing archive limits the full-wave exercise; later committed result documents may be a separate artifact-source arm, never relabeled structured Mesh coverage.

The acceptance threshold is correctness first: zero invented candidate/GO/owner/acceptance claims; all required missing evidence explicit; all supported facts reproducible. Do not demand a guessed percentage from an incomplete archive. For a prospective complete wave, measure authored ledger bytes/turns, generated snapshot commits, duplicate result copying and time to locate operative evidence. Adopt the authoring path if it preserves obligations while reducing manual projection work; no numeric savings threshold is claimed from this study.

Rollback: stop ledger authoring at a recorded cut, preserve/export the log, and generate one human-readable fallback baseline. Use a separately named manual continuation document with its provenance; do not maintain two competing `ledger.md` authorities. Tasks and messaging need no rollback because they were never changed by ledger events. A reader-only deployment can simply be disabled.

## 10. Required tests and concrete review gates

These are proposed future tests, not tests run against live repositories in this research task. Regression fixtures should record the historical source/cause; no performance/build gate is warranted for this Markdown-only deliverable.

| Acceptance criterion | Minimal meaningful test |
|---|---|
| Deterministic current fold and past replay | Entry A; amend B naming A; compare current and cut-at-A; same bytes/cut/version twice produce identical Markdown/JSON; late old-clock event appears at its admission sequence. |
| No competing heads | Race two creators for one key, two amendments to one head, and amend versus tombstone. Exactly one wins each conflicting precondition; loser has no committed event. |
| Retry correctness | Lose response after commit; retry same ID returns original receipt even though expected head is now old. Same ID/different body fails; restart/rebuild cache preserves result. |
| Authorship and authority | Original author edit, unauthorized cross-author edit, authorized lead override, old/inactive author, scope declaration by wrong owner, reviewer-source ruling cited by a note. Original attribution survives; task/ruling files byte-identical. |
| Small event and honest limits | Over-limit body/reference count is rejected; UTF-8/newline/pipe/control-char inputs round-trip or escape correctly; qualifier cannot disappear under clipping; `remaining: unknown` differs from explicit none. |
| Source identity and joins | Reused task number/team name after recreation; T6a alias; reassignment; source/landing mismatch; completion-before/after ruling; T9 review-on-T2; budget-only/negative rulings. No false accepted-candidate join. |
| Historical evidence preserved | R38 equal-number/different-instrument observations stay distinct; transcription correction supersedes only the incorrect claim; later clean run does not erase earlier FAIL; R41 exception narrows only its named item. |
| Durable committed prefix | Crash before/during append, after line sync, around manifest replacement/directory sync; disk-full and sync failures. No acknowledged event lost; no unpublished suffix rendered. |
| Corruption and cursors | Malformed committed row versus torn uncommitted tail; multibyte split; source truncation, same-size replacement, missing segment, unknown schema; stale epoch. No skip/rewind presented as complete history. |
| Pull purity | Run render/history/offline replay while trapping filesystem writes and daemon/network/tmux access. Zero task/activity/inbox writes and no service dependency. Empty query creates no directories. |
| Boundary reproducibility | Copy bundle to a fresh temp root, remove access to original runtime root, rebuild caches, rerender same bytes. Missing required artifact produces an explicit failure/gap. |
| Delivery obligations | Golden cards for measure/no commit, diagnose/limits, implement/source+landing/red evidence, hero two-lens review, numbered findings/questions/scores, per-root zeros/unmeasured dashes. Links resolve within declared retained evidence. |
| Scope read discipline | Late old event after mark, multi-page walk, hidden history, unresolved ruling, authority-declared scope outcome. Marks only returned/seen items and resolves nothing. |
| Role command validity | Extract every published ledger command example, substitute fixtures, execute actual parser/handler success and failure paths. No missing identity flags or future commands under old pin. |

No new service, watcher, automatic snapshot-per-event loop, LLM summarizer, task-state reconciliation writer, inbox body archive, private-review publication path, distributed log merge, or full historical Markdown importer belongs in these increments.

## 11. Reproduction and evidence inventory

The following **research-only** standard-library script reads the archived bytes without extraction, prints the census, and reconstructs the trial projection in memory. It runs no Mesh commands and reads no live team/account state. Re-running the live-wave census later requires the captured hash to match; otherwise report a new population, not changed results for this one. The simple table split is verified for these artifacts, not proposed as a general Markdown parser.

```python
from pathlib import Path
import hashlib, json, re, tarfile

repo = Path.home() / "projects/taurjob"
archive = repo / "docs/wave-1/mesh-archive"
ledger = (repo / "docs/wave-1/ledger.md").read_bytes()
assert hashlib.sha256(ledger).hexdigest() == (
    "d9272cf638e3071509f4b6767e57cd674257a43a768b5ce5df2a62726e7656bb")
rows = []
for line in ledger.decode().splitlines():
    if re.match(r"^\| T\d", line):
        cells = [c.strip() for c in line.strip("|").split("|")]
        assert len(cells) == 8
        rows.append(cells)
columns = [sum(len(row[i]) for row in rows) for i in range(8)]
print("rows, column characters:", len(rows), columns)
print("T25-T28 characters:", sum(sum(map(len, r)) for r in rows
                                     if re.match(r"T2[5-8]\b", r[0])))
tasks = {f.stem: json.loads(f.read_text())
         for f in (archive / "tasks").glob("*.json")}
hits = []
for row in rows:
    match = re.fullmatch(r"T(\d+)(?: / .+)?", row[0])
    tid = match.group(1) if match else None
    if tid not in tasks:
        continue
    task = tasks[tid]
    encoded = json.dumps(task, ensure_ascii=False)
    for col in (0, 2, 6):
        supported = (col == 0 or
                     (col == 2 and row[col] == task.get("owner")) or
                     (col == 6 and re.fullmatch(r"[0-9a-f]{7,40}", row[col])
                      and row[col] in encoded))
        if supported:
            hits.append((tid, col, len(row[col])))
print("factual hits:", len(hits), "characters:", sum(x[2] for x in hits))
with tarfile.open(archive / "taurjob-team-archive.tar.gz") as tar:
    journals = {}
    for name in ("workflow_events", "protocol_index", "task_mutations"):
        body = tar.extractfile(f"taurjob-team/state/{name}.jsonl").read()
        journals[name] = [json.loads(line) for line in body.splitlines()]
        print(name, len(journals[name]), hashlib.sha256(body).hexdigest())
events = journals["workflow_events"]
latest = {}
for event in events:
    if event["eventType"] == "task_state_updated":
        latest[event["task_id"]] = event
out = ["# Archived wave-1 structured evidence — incomplete early cut", "",
       "Generated by research-only reader; no ledger prose supplied to generation.", "",
       "| Task | Owner at snapshot | Snapshot status | Last workflow status | Rulings | Completion evidence |",
       "|---|---|---|---|---:|---|"]
for tid in sorted(tasks, key=int):
    task = tasks[tid]
    metadata = task.get("metadata", {})
    completions = [e for e in events if e["eventType"] == "task_completed"
                   and e["task_id"] == tid]
    ref = completions[-1]["eventId"] if completions else "unavailable"
    out.append("| " + " | ".join([
        tid, task.get("owner", "—"), task["status"],
        latest.get(tid, {}).get("task_status", "unavailable"),
        str(len(metadata.get("rulings", []))), ref]) + " |")
out.extend(["", "## Retained completion statements", ""])
for event in events:
    if event["eventType"] == "task_completed":
        out.extend([f"### Task {event['task_id']} · {event['eventId']}", "",
                    event["summary"], ""])
projection = ("\n".join(out) + "\n").encode()
print("projection:", len(projection), hashlib.sha256(projection).hexdigest())
# Optional display: print(projection.decode()). No write is needed.
```

Expected census: rows 29; column characters `[155, 544, 553, 874, 1500, 30576, 203, 17754]`; dominant four 44,706; factual hits 27 / 219 characters; journal counts 321 / 53 / 98; projection 3,435 bytes with the §2.4 digest. Manual section taxonomy uses the §2.1 named section boundaries; its semantic classifications are explicitly judgments rather than automated proposition labels.

| Additional retained source | SHA-256 |
|---|---|
| Archive tarball | `60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1` |
| Archived workflow JSONL | `907075d8fbaa2d9cbaa5424ecbc17880bf7aff135821134585d8d6b56c3dbe38` |
| Archived protocol-index JSONL | `0f26b0e3fb87db07544bc274f2ab1d0fcd5af51a077147d9dd7ef8ced363849c` |
| Archived task-mutation JSONL | `c5f3ce38628b2cb1cad7a0e2fbc235adffa2777bfeb3c717cbedcfbc038be194` |
| Threads study | `3c5a7071f0959b59be0b56d446a2090f9e2689add9355f4da5800c40438a68ca` |
| Messaging overhaul study | `3b12c0a5186c84d36095e67d116dfe025ed621d11ef68b834a4e859f8cfc3e7b` |
| Delivery standard | `e0195b65b27a91c85a543b5f44a95e06199d50fc01e0d2a0955b290fcc622511` |
| Phase-0 seed (`mesh-core-team.md`) | `99f73887a55214816adc9e60b89e82d3e0561a109c672a58d86fff6f01b5eac8` |
| Commissioning brief | `9205aafd9e426a7237ae570579ccd699d7eb945598906d19a9324c2470387a7d` |

## 12. Open questions and implementation decisions to ratify

These are not requests to interrupt the live wave. Each has a bounded post-wave resolution and a safe default.

| Question | Confidence / default | Resolution before activation |
|---|---|---|
| Is there a retained final structured wave-1 generation outside the inspected archive? | **U.** Do not search/mutate live roots to improve this study's number. Current coverage is early-window only. | Operator supplies/identifies a closed exported generation; rerun the proposition census as a separately labeled population. |
| How much remaining/outcome prose is already in later completion summaries? | **U** full-wave percentage; high confidence that early examples are already there. | Gap matrix on final exported wave-2 records and selected artifacts. Add annotation fields only for evidenced gaps. |
| Where is the immutable acceptance/scope authority manifest owned? | **I, medium.** Approved wave packet with explicit recorded revisions; no live-roster inference during replay. | Choose the smallest existing packet/assignment seam; validate actor, override and reassignment rules before writer activation. |
| Should a future typed candidate/review association live in ruling metadata instead? | **I, medium.** Ledger may carry an attributed link; authoritative acceptance remains a ruling. | Prefer a deliberate ruling-schema extension if every review needs it. Avoid making a mandatory ledger entry the only way to count acceptance. |
| What is the archive/bundle retention location across account changes and team removal? | **I, high need; path U.** Commit compact boundary evidence or a durable content-addressed bundle with verified retention. | Demonstrate offline replay after original team root is unavailable. A `/tmp` link alone fails. |
| Does the target filesystem honor the specified lock/sync/rename contract? | **U** until crash tests. POSIX/WSL writer only initially. | Test the actual supported storage; do not promise UNC or native Windows writer equivalence. |
| Is single-segment full replay fast enough? | **I, likely for this small record; latency U.** Start there without polling. | Benchmark realistic event count/body distribution before adding rotation/index complexity. |
| Does the generated default expose enough current evidence without giant cells? | **I, medium.** Compact table plus anchored declarations and explicit history. | Review T25–T28 and R41 with the acceptance owner; reject hidden qualifications or invented `remaining: none`. |

**Completion assessment:** the commissioned research is delivered: measured corpus/coverage bounds, source-authority decision, minimal event shape, authority/CAS/fold/replay rules, storage/recovery, execution-validatable future CLI contract, deterministic rendering and snapshots, consumer joins, migration and bounded test plan. The live wave remains untouched. No implementation, commit, deployment, or cost reduction is claimed.

---

**Review-pass amendment (orchestrator, 2026-09-07, per the operator
constraint recorded in the brief after this lane launched).** §7's
authoring intake is amended: `mesh ledger entry --file <entry.md>` —
markdown with YAML front-matter (envelope/reference fields above a
literal prose body) — is the AUTHORED path; `--file <entry.json>`
remains for programmatic emitters that write no prose. Both normalize
into the same canonical validated JSONL, which stays exclusively the
writer's output format. Rationale: authored prose must cross no
escaping layer (the constraint's bash-vs-JSON point); the study's 4 KiB
body bound shrinks the escaping surface but does not remove it, and
front-matter costs the parser one small reviewed adapter. Everything
else in §7 (file-supplied IDs, no argv prose, explicit root/identity,
structured errors, the execution-validation gate) stands as written and
already satisfies the constraint's intent.
