# One assignment, one complete contract card

## Result

**[I — UNVERIFIED design] Generate the assignment notice from the task record, with the five contract lines intact, an assignment-token header, and conditional execution context.** The lead supplies the five-line contract at creation; assignment binds it to the owner and a fresh generation and supplies any generation-specific wait. Creation-time reference/budget drafts are consumed only by the first assignment (the writer/lifetime table below governs subsequent assignments). The same rendering supplies the directed inbox notice and optional inspection surfaces. GO is a subsequent authorization change with its own bounded rendering, not a second copy to suppress. This follows the direction in `docs/design/mesh-next-adjudication.md:11`, `:15`, `:66` and the preservation requirement in `docs/design/field-test-wave2/machinery-review-astra.md:210`. It remains a proposal: acceptance and the conformance checks under Recommendation would settle implementation readiness.

### Evidence contract

| Label | Meaning in this document |
|---|---|
| **S** | Source bytes inspected, archive computation reproduced, or named isolated binary observation. Source inspection is not a running-app test; a probe is identified explicitly. |
| **D** | Documented requirement in the cited local governing document; not proof that the implementation enforces it. No vendor or live-service claims are used. |
| **P** | Prior study finding retained with its original population and estimator; not independently observed model behavior. |
| **I — UNVERIFIED design** | Proposed behavior or inference. The cited evidence motivates it; the proposed acceptance checks, not this document, would establish it works. |
| **U — UNVERIFIED** | Missing evidence or an unresolved interpretation. Each open question names what would settle it and a safe default. |

**[S] Inspection scope:** checkout `/home/mstie/projects/taurhaus`, branch `main`, round-1 HEAD `9dd8427e2c084aa1a47a63d338083f1df21c5f78` (read-only `git branch --show-current` / `git rev-parse HEAD` output). `CLAUDE.md` was read first. Only `.check-logs/mesh-next-phase0/` was written. The probe runner uses an explicit temporary root and `env={}`, starts only `/home/mstie/.local/bin/mesh`, and kills/waits its own child in `finally` if it has not exited (`probe-mesh.py:8`, `probe-more.py:4`, paths relative to this report). No model CLI, daemon, tmux, load test or repository gate was started. These are script/command observations, not an OS-level access audit.

**[S] Binary boundary:** `mesh-probes.log:1` records the executed version command and real output: version `0.2.29`, commit `6789201c5511b51be704fe30c6e4d025f3e64f8c`, `git_dirty: false`, protocol `1`, schema `1`. This matches `src-tauri/resources/mesh.lock.json:2`. The report does not claim that the running team uses this binary or that a product gate passed.

**[S/D] Citation convention:** unqualified `main.rs`, `task_lifecycle.rs` and `idle_monitor.rs` mean `/home/mstie/projects/mesh/src/` files; `standard` means `docs/team-delivery-standard.md`; `adjudication` means `docs/design/mesh-next-adjudication.md`; `machinery-review-astra.md` means `docs/design/field-test-wave2/machinery-review-astra.md`; `overhaul` means `docs/design/mesh-messaging-overhaul-research.md`. Scratch evidence filenames resolve beside this report; the pair inventory's `source:N` means `nine-pairs-source.txt:N`. Those local paths and lines are the evidence, not external citations.

### Field-preservation matrix

**[D] Required inputs remain the five lines** from `docs/team-delivery-standard.md:19`. Deadline and effort are optional; a reason is not required (`:123`). **[I — UNVERIFIED design]** The matrix specifies record locations and output order. New structured context is optional; when a lane already declares a wait, budget, or review manifest, completeness checks apply to that declaration, not to every task. The evidence for those conditional obligations is the standard at `:53`, `:66`, `:90` and adjudication at `:61`.

| Field | Today's carrier [S unless D noted] | Proposed record carrier [I — UNVERIFIED design] | Rendered line / preservation rule [I — UNVERIFIED design] |
|---|---|---|---|
| Objective | Task subject/description; full outcome in prose. Generated card uses subject, not description (`/home/mstie/projects/mesh/src/idle_monitor.rs:891`). | Existing `description` holds the outcome sentence; `subject` remains the short title. No second objective key. | `Objective: {description}`; never truncate operative text. Existing longer descriptions remain lossless context until explicitly normalized by the author. |
| Deliverable | `metadata.deliverable`, with shorter card variants; detailed artifact content, bounds and exclusions often only in prose (`nine-pairs-source.txt:8`, `:30`, `:70`). | Existing `metadata.deliverable`. | `Deliverable:` includes exact paths/output contract and all lane-specific acceptance detail not solely delegated to an immutable packet. |
| First action | `metadata.first_step` / legacy `firstStep`; richer prose action and constraints (`main.rs:310`, absolute Mesh source path below). | Continue writing `first_step`, reading both spellings. | `First action:` imperative; in a wait, permitted pre-read now and execution after release are distinguished explicitly. |
| Completion signal / response expectation | `completion_signal` / `completionSignal`; often contains acceptance route as prose (`main.rs:338`; archive inventory). | Existing `completion_signal`. | `Completion signal:` ready state/message, result details, BLOCKED alternative, and no pure acknowledgment. Do not retain the unconditional “Only reply after work” when blocking is the prescribed signal. |
| Review route | Usually embedded in completion metadata plus richer prose; no field in `AssignmentContract` (`main.rs:200`). | **New** `metadata.review_route`, one text field containing work kind, acceptance owner, reviewers, and seam/handoff only (standard:25). Stopping/isolation belongs to optional `assignment_context.review_manifest` rendered under Context and References (standard:90; adjudication:51). It represents the existing fifth line, not a sixth required input. | `Review route:` complete and self-contained; no peer findings added by projection. |
| Task/owner/generation/assigner/time | Existing task `id`, `owner`; `assignment_id`, `assigned_by`, `assigned_at` (`task_lifecycle.rs:244`). Cards carry full token. | Preserve these exact keys. Renderer gets team/root identity from the resolved task source; no new user-entered ID. | Header names task, owner, full assignment token and issuer/time. Task ID comes from creation's actual result; never predict an ID. |
| Effort and reason | `effort`, `effort_why` / `effortWhy`; surfaced on card/get (`idle_monitor.rs:863`, `main.rs:1583`; probe 8–11). None in the 11 wave-1 snapshots (`archive-task-inventory.txt:1`). | Same keys, optional independently of five-line contract. Preserve supplied reason verbatim after safe display escaping. | `Effort: {level} — {reason}` if supplied; omit absent reason, never fabricate one. Requested effort is not proof runtime effort took effect. |
| Deadline | `deadline_minutes`, optional (`main.rs:3223`; probe 8–11). | Same key and current units. | `Deadline: {minutes} minutes` only when supplied; do not infer a fresh deadline on recovery. |
| Accepted base / candidate / rubric / packet hash | References currently can be text in action/completion/description; pair 9 has rubric check count and paths but no frozen rubric digest (`nine-pairs-source.txt:177`). D: freeze review manifest (`standard:90`). | Optional `metadata.assignment_context.references[]`, each `{role, locator, revision, digest?, availability, retain_until?}`. Roles distinguish `accepted_base`, `candidate`, `rubric`, `packet`; revision is immutable commit/object identity, digest covers non-Git bytes or a packet manifest. | `References:` list each role independently with exact locator and revision/digest. No “latest”, no resolving a mutable worktree path to new bytes. A pending review candidate can be shown as pending before GO; pending is not reviewed or accepted. |
| Release condition and operative wait | Text waits in both T9 generations; modern mesh `awaiting_go` equals current assignment token (`main.rs:3428`). D: awaited artifact/owner/generation/release condition (`standard:53`). | Keep `metadata.awaiting_go` token and existing lifecycle/dependencies. Optional `assignment_context.wait = {kind, owner, artifact_ref, condition, since, permitted_action}` describes the existing wait; it is not a competing state authority. | `Execution: WAIT…`, owner, age/as-of, condition, allowed pre-read, no execution. Only report committed lifecycle state. A matching `awaiting_go == assignment_id` gates WAIT; `assignment_context.wait` describes it but never gates execution independently. Conflicts are annotations with an existing BLOCKED route, as specified below. |
| Budget counting rule and tool | Wave-1 prose has several line ceilings; M0 exclusions are in record too. D: counting semantics in standard `:66`; tool requirement in adjudication `:61`. | Optional `assignment_context.budget = {baseline_ref, owned_paths, counting_rule, script_ref?, exclusions, ceiling}`. | `Budget:` declared rule and script, explicit exclusions and exact base. Missing historical script is unknown, not an invented command. |
| Checkout, constraints and related obligations | Often prose-only: read-only source, no personal data, forbidden paths, predecessor closeout, next seam (`nine-pairs-source.txt:47`, `:67`, `:93`). | Optional `assignment_context.constraints[]`, `checkout`, `related_actions[]` (each names its other task and release condition). | `Context:` needed safety/scope facts, without recopying standard doctrine. Related task closeout never implicitly blocks or authorizes this task. |
| Machine orchestration kind/lane/criticality | `work_kind`, `lane_id`, `criticality` in archived snapshots and creation flags (`archive-task-inventory.txt:1`; probe 8). | Preserve keys and vocabulary. Do not equate mesh `verification/runtime/docs` with standard `measure/implement/spec-delta`. Semantic work kind stays in review route. | Optional routing annotations; not additional lead-required lines. |
| Parent task links | `--parent` writes `metadata.parent_task_ids[]` (`main.rs:3190`; round-1 probes 40–41). | Preserve key, array order and IDs; creation owns it. | Context: labelled parents; not implicitly a dependency or acceptance. |
| Anchor links | `--anchor` writes `metadata.anchor_task_ids[]`, not a new `anchor` key (`main.rs:3191`; round-1 probe 40 output). | Preserve `anchor_task_ids`; inspect aliases without renaming persisted fields. | Context: labelled anchors and their IDs; not proof of delivered artifacts. |
| Scaffold classification | `scaffold_class` (`cli.rs:387`; probe 40). | Preserve exact vocabulary. | Context: scaffold class, separate from semantic work kind. |
| Sunset decision | `sunset_decision` (`main.rs:3193`; probe 40). | Preserve exact decision. | Context: sunset decision; a recorded plan, not execution authority. |
| Sunset owner | `sunset_owner` (`main.rs:3194`; probe 40). | Preserve named owner. | Context: sunset owner; never infer from assignee. |
| Sunset trigger | `sunset_trigger` (`main.rs:3195`; probe 40). | Preserve exact trigger. | Context: sunset trigger verbatim; no implicit cleanup. |
| Rulings | Archive `metadata.rulings` (`archive-task-inventory.txt:1`, `:11`). | Keep canonical historical data and ref linkage. | Context or Record metadata only for authorized rulings relevant to the generation. Peer review rulings withheld from unlocked reviewers, with restricted-coverage notice; never inferred from latest verdict. |
| Verdict | Archive `metadata.verdict` (`archive-task-inventory.txt:1`). | Preserve, never copy into a new generation's acceptance. | Authorized inspection: labelled recorded verdict and reference/scope if present; absent linkage is unknown. Restricted review projection excludes peer verdict and derivatives before rendering. |
| Artifacts | Archive `metadata.artifacts` (`archive-task-inventory.txt:11`, `:13`). | Preserve artifact identities without automatic reads. | References for authorized artifacts; exclude restricted peer artifact locators as well as contents. No later snapshot backfill into pre-GO examples. |
| Review summary | Archive `metadata.review_summary` (`archive-task-inventory.txt:1`); cleared on assign (`task_lifecycle.rs:15`). | Keep current-generation semantics. | Authorized inspection only; excluded on new assignment because cleared, and on restricted reviewer projections because it can contain a verdict derivative. |
| Review request time | Archive `metadata.review_requested_at` (`archive-task-inventory.txt:1`); cleared on assign (`task_lifecycle.rs:16`). | Preserve recorded time, never substitute delivery/read time. | Context: review requested at, when present and authorized; not the wait's `since` unless explicitly recorded as that decision request. |

**[I — UNVERIFIED design] Unenumerated-key rule:** preserve unknown keys in storage and JSON under existing names/types. After the same authorization filter, render unknown keys in `Record metadata:` with their full labelled value and stable key order; do not silently drop a potentially operative field. For restricted reviewers, unknown fields are denied by default until classified and a generic “unclassified metadata withheld pending access review” coverage annotation is shown (without exposing key names or counts if those reveal peer work). Access classification/author repair, not a raw fallback, settles whether such fields may become actionable. This default cannot leak peer artifacts, opinions, verdicts or derivatives (adjudication:51). Source coverage is not a reason to weaken isolation.

**[I — UNVERIFIED design] Data shape and validation:** `assignment_context` is one additive optional object, not a new editable contract database. Existing flat metadata remains canonical for its fields. Five-line values are authored once and are not parsed out of sent prose. Creation may retain legacy drafts, but a combined-card assignment must have all five nonempty lines. No silent subject-as-objective substitution in new contracts. Legacy records missing a line get an explicit “not recorded” display and author repair before new combined-card assignment. Do not require that repair merely to inspect an old task. This is an intentional validation/behavior change requiring the compatibility review below, motivated by `main.rs:200`, `:310` and standard `:19`.

### Context writers and assignment lifetime

**[S]** `ASSIGNMENT_CLEAR_KEYS` is a top-level metadata-key list (`task_lifecycle.rs:8`), used on every assign at `:267`; `merge_task_metadata` removes exact keys before inserting new values (`:214`). It does not understand dotted subkey paths. **[I — UNVERIFIED design]** Add the exact top-level entry **`"assignment_context"`** to that list, not dotted strings. Capture the previous object before clearing; reconstruct only the task-scoped whitelist below and explicitly supplied/derived generation data. Existing clear entries, especially `awaiting_go`, stay intact. Omitted generation keys must be absent, never copied. No new context key has shipped in this lane.

| `assignment_context` subkey [I — UNVERIFIED design] | Lifetime and reset | Writer command / input contract (proposed extensions unless stated otherwise) |
|---|---|---|
| `checkout` | Task-scoped; copy explicitly across assign. Author must correct changed checkouts before new assignment. | Task create authors optional structured context; task assign may explicitly replace it with an audited correction. |
| `constraints[]` | Task-scoped; explicit carry whitelist. Narrower generation constraints must be restated, never silently removed. | Task create; task assign explicit audited replacement. Existing broader repository/packet constraints always bind. |
| `references[]` | Generation-scoped, cleared on reassign. Initial unassigned task may hold creation drafts, consumed only on first assign; no task-scoped candidate defaults. | Task create authors draft refs; task assign freezes supplied refs for that generation; proposed TaskCommand::Go supplies/finalizes pending candidate/rubric/packet refs at release. All identity fields (`role`, `locator`, `revision`, `digest`, `availability`, `retain_until`) share this lifetime. |
| `budget` | Generation-scoped including `baseline_ref`, `owned_paths`, `counting_rule`, `script_ref`, `exclusions`, `ceiling`; initial creation draft consumed only on first assign. A reissue does not authorize baseline recutting (standard:66). | Task create or task assign supplies full budget. Existing task ruling (budget_raised) records old/new/reason before crossing; proposed same writer materializes the approved ceiling without changing original baseline/counting semantics. Reassignment must explicitly restate an ongoing leash and its ruling provenance. |
| `wait` | Generation-scoped, including `kind`, `owner`, `artifact_ref`, `condition`, `since`, `permitted_action`; never inherited or auto-retokened. | Task assign receives an optional typed context payload **beside** existing `--awaiting-go`. The caller supplies condition/artifact/owner and original decision-request `since` (or explicit unknown); only a wait newly requested by this mutation may use commit time. The writer atomically sets the UUID `awaiting_go` and wait descriptor before delivery. Creation cannot set a live wait. Task block owns blocked-state recording and its descriptor; proposed TaskCommand::Go clears the released descriptor and matching marker together. |
| `related_actions[]` | Generation-scoped; each `task`, `release_condition`, action and any generation ref cleared. | Task create draft / task assign explicit payload. No inferred completion of an older obligation. |
| `review_manifest` | Generation-scoped: candidate, rubric, packet, allowed evidence, lens, isolation and stopping condition freeze together. | Task create draft / task assign / proposed TaskCommand::Go before review release; changes to already executing review scope require a new assignment and admin reason, not a silent overwrite. |
| `previous_assignment_id` | Generation-scoped; recomputed on every reissue from previous committed token; absent on first assignment. | Task assign, machine-maintained; never user supplied. |
| `assignment_change_reason` | Generation-scoped; newly supplied `--admin-reason`, not an inherited prior reason. | Task assign, from the authorized reissue's actual admin reason; historical workflow reason is labelled separately when admin reason is unavailable. |
| `changed_fields[]` | Generation-scoped; recomputed normalized diff at reissue. | Task assign, machine-maintained from previous/current authorized records; no peer verdict text in diffs. |
| Any future subkey | Generation-scoped by default and omitted on reassign; no whitelist inheritance without schema review. | Explicitly named writer required before support; unknown historical values remain inspectable subject to access policy. |

**[I — UNVERIFIED design]** Creation accepts only draft content for the first generation. On assign, capture task-scoped fields and (only if no previous assignment exists) creation drafts, clear the whole object, then construct the new object from that whitelist plus explicit current input and derived linkage. Snapshot it with the assignment event for later historical rendering; do not consult the mutable latest object for a prior generation. Use a typed structured payload, not the current string-only `extra_metadata` shortcut (`task_lifecycle.rs:254`). Missing context remains optional. A declared wait requires the standard's conditional details; it is not a sixth universal contract line. Reader/rendering code never writes lifecycle state. Acceptance requires a generation-1 → generation-2 fixture proving old references/wait/budget/previous token cannot survive omission, while checkout/constraints do carry (standard:53; findings 6–7).

### Recorded execution and conflict routing

**[S]** Mesh's GO-wait authority compares `metadata.awaiting_go` to the current assignment UUID (`main.rs:3428`; `idle_monitor.rs:716`); declared member blocks with a reason also suppress idle nudges (`idle_monitor.rs:702`). **[I — UNVERIFIED design]** The renderer reads those existing states; it creates no hold, deadline exception, or transition. Precedence is:

- Matching `awaiting_go` means `Execution: WAIT`, even if `assignment_context.wait` is absent or disagrees; show missing/conflicting description and the existing BLOCKED repair route.
- Absent or stale `awaiting_go` cannot become WAIT merely because a context object says wait. Report the existing lifecycle status and annotate the mismatch. If execution is forbidden by the authored instruction, retain that restriction in First action/ACTION REQUIRED and route an explicit block; never invent machine suppression or show an unconditional start. A recorded block remains a block independently of the GO marker.
- An unavailable historical marker is reported as unavailable, never as READY or released. Completed/review states remain their recorded states; recovery does not restart them. Only a consistent executable assignment with no recorded wait gets a concrete start heading.

**[D/I — UNVERIFIED design]** For branch, budget, identity or wait conflicts, show both values in Reconciliation. The assignee must use the existing task block lifecycle plus its named BLOCKED completion signal, naming task, assignment generation, awaited artifact/ruling, owner and release condition (standard:53–59; current block help in probe 38). The lead reconciles the conflict through that lifecycle. Until that write happens the annotation must say it is an obligation, not recorded state. Do not reinterpret T8 as a stricter temporary total-line ceiling: no renderer may change fixed counting semantics. Historical examples below preserve commands as evidence only.

### Deterministic card template

**[I — UNVERIFIED design; basis: standard:19, :31, :53, :90; adjudication:15, :51, :77]** Render stored values in the following fixed order. Determinism is over the committed record, authorized audience, renderer revision and explicit as-of time; a notice freezes as-of at its triggering mutation, while an inspection may request a newer as-of. A displayed age never changes the underlying wait or authorizes execution. Omit genuinely absent optional sections; escape control characters without shortening identifiers or dropping constraints. Multiline value continuations are visibly indented, never interpreted as new headings. Do not append the full task description a second time. Identity and contract facts come from the same committed generation. Template annotations in braces/angle brackets are schema variables, not supported CLI flags. Square brackets on optional section lines mean omission when absent; those delimiters are not emitted. The enclosing header brackets are literal. The ten examples use the same `render()` function (`render-pairs.py:10`).

```text
[#<task id> <subject> · owner: <owner> · team: <resolved team>]
Assignment: <full assignment_id> · issued by <assigned_by> at <assigned_at>
[Effort: <level> [— <supplied reason>]]
ACTION REQUIRED: <start permitted action: concrete verb/file | pre-read only; execution waits | resume stated stage | report recorded blocker>
Execution: <recorded lifecycle state; WAIT if current-token awaiting_go or recorded block; RECORDED status/unknown if historical authority is unavailable>
Objective: <description>
Deliverable: <metadata.deliverable>
First action: <metadata.first_step; explicit now/after-release distinction>
Completion signal: <metadata.completion_signal; including BLOCKED and response expectation>
Review route: <metadata.review_route>
[References: accepted base …; candidate …; rubric …; packet …]
[Deadline: <deadline_minutes> minutes]
[Budget: baseline …; paths …; counting rule/tool …; exclusions …; ceiling …]
[Context: checkout …; constraints …; related task obligations …]
[Change: supersedes <old assignment> because <admin reason>; changed fields …]
[Reconciliation: visible conflicting source values; existing BLOCKED lifecycle route, not a new state]
[Record metadata: authorized unmapped keys as labelled lossless values; unavailable/restricted fields labelled]
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

**[I — UNVERIFIED design] Fresh assignment:** validate the five lines, allocate a task ID at creation and the existing assignment UUID at assignment; commit the record before generating the notice. Persist optional previous-generation/reason linkage on the current task for its recovery rendering, using the existing supersession/admin event as evidence rather than inferring chronology. Store the immutable reference *identities*, not peer verdict bodies. The current writer already commits then formats the task (`main.rs:3450`, `:3523`); task assignment and supersession events are separate (`:3481`, `:3495`). The new complete rendering and recovery fields are not yet implemented.

**[I — UNVERIFIED design] Reassignment:** always show the new full token, changed owner/action/route/references, old token and admin reason. A same-owner replacement still replaces the generation. Prior cards become historical evidence; their token cannot authorize the new work. Probe 16 rejected a missing reason; probe 18 task-get exposed the new token and probe 19 rejected the old token. Probe 17 printed only the inbox message ID, not the assignment token (`round1-token-check.log:2`; `main.rs:3561`). Do not copy a predecessor's wait marker to the new token implicitly; the lead must deliberately preserve or replace the wait.

**[I — UNVERIFIED design] Resume:** a context recovery within the same owner, generation and stage preserves its token, wait and immutable references. Show `Resume` with the stored next action/cursor and current operative contract; do not create another assignment merely to redisplay it. A reopened completed task or genuinely new stage contract uses a fresh assignment; probes 22–24 exercised completion and reopen with an admin reason. If the source cannot distinguish resume from reissue, display the recorded state plus an identity conflict and require the assignee to record a block and send its BLOCKED signal with task/generation/artifact/owner/release condition. The renderer cannot introduce an execution hold. This follows the standard's restart identity check (`:45`) without imposing a mandatory card read in every turn (adjudication `:15`).

### Pre-GO and GO are different obligations

**[I — UNVERIFIED design] Pre-GO** delivers the full contract, token, wait owner/release condition, permitted pre-read and pending/frozen references. It must never say “start execution now.” **GO** names the same current token, the issuing authority, the exact released condition and newly frozen candidate/rubric/packet identities, followed by the executable first action and changed completion/review terms, if any. Unchanged terms may be identified by the immutable contract generation; all information needed to act on the release is delivered in its body. Cold recovery may render the full current contract, counted as a separate recovery exposure. Basis: standard `:53`; machinery review `:210`; pair 9 `nine-pairs-source.txt:171`.

```text
ACTION REQUIRED: GO for task #<id>, assignment <full token>, owner <owner>.
Released by <authority> at <time>: <specific condition now satisfied>.
Accepted base: <immutable id>; Candidate: <immutable id>;
Rubric: <immutable id>; Packet: <immutable id, if applicable>.
Changes since pre-GO: <explicit delta, or none>.
First action now: <executable action against those references>.
Completion / review: <operative response, route, isolation and stop condition>.
Other waits: <remaining waits, or explicitly none>.
```

**[S] Current GO is not that renderer.** Probe 37 ran the existing release command; the observed inbox counts were identical before and after (`probe-observations.txt:1`): `{'seat2.json': 4, 'lead.json': 4, 'seat.json': 3}`. The handler removes `awaiting_go`, records workflow state and notifies the lead; it does not generate an assignee GO body (`/home/mstie/projects/mesh/src/main.rs:3316`). **[I — UNVERIFIED design]** Connect the proposed release rendering to the existing directed notice mechanism after the authorized state transition. This changes command response/delivery obligations and must be explicitly reviewed; it does not replace the transport or scheduler. Never treat a cleared field or a Scope read as proof the assignee received GO (adjudication `:15`).

**[I — UNVERIFIED design] GO surface decision:** attach new release rendering to a **new TaskCommand::Go lifecycle command**, named `task go` in the proposed CLI enum, not to the low-level update path. This is proposed syntax, not a runnable example or an implemented command. It requires the expected assignment UUID, verifies lead authority/current owner/dependencies and frozen refs under the mutation lock, atomically releases only that generation's wait, records the release, then sends its bounded body through existing directed delivery. Failure to deliver remains explicit; commit is not receipt. Legacy `task update --go` remains unchanged until the paired contract review decides its migration/deprecation; do not claim it is generation-safe. The future generated-release path must not call that unguarded handler as a substitute for the compare-and-commit check. Acceptance owner: paired Mesh/Taurhaus contract owners and orchestrator (overhaul:217; main.rs:3273, :3316).

**[S] Warning precision:** the current low-level warning is conditional on status/owner updates (`main.rs:3282`); the bare GO probes 13/20/37 produce no such warning. This qualifies finding 10's premise without weakening the generation-check requirement. Current GO takes only `go: bool` and removes the marker without comparing a supplied generation (`main.rs:3273`, `:3322`).

### Historical T9 GO through the release template

**[S] Source:** `t9-go-evidence.txt:1` identifies judge-astra-1 inbox index 5 and the release timestamp; `nine-pairs-source.txt:169` and the assignment/supersession events identify its current task-9 generation. The fresh archive verifier checks the row, sender, timestamp and task-9 assignment identity, without opening any candidate or peer-review bytes (`verify-report.py:1`). **[I — UNVERIFIED design]** This is an offline template projection of that real release, not a new authorization or evidence that today's mesh delivered a generated GO. Full hashes, accepted base and separate packet/rulings revisions are not recovered; a future writer must resolve/freeze them before authorizing new review (standard:90). The template reports historical gaps instead of silently upgrading abbreviated refs. Nothing from this GO is backfilled into the pre-GO cards.

```text
ACTION REQUIRED: GO for task #9, assignment 49f0e0e3-6833-40f8-906a-78419909a5b1, owner judge-astra-1.
Released by lead-taurjob at 2026-09-06T19:58:00.249Z: T9 go 8f97e28; T2 design material delivered for review.
Accepted base: not recorded; Candidate: 8f97e28 (archived abbreviated ref, full object identity unverified), ~/projects/taurjobs/docs/wave-1/design.md (373 lines) and docs/wave-1/mockups/index.html linking 01-configure.html, 02-run-progress.html, 03-results.html with shared tokens.css and mock.js;
Rubric: docs/wave-1/acceptance.md at 6a44056 (archived abbreviated ref, full identity unverified), plus docs/wave-1/rulings.md R1 to R10 (separate immutable ruling revision not recorded); Packet: no separate immutable packet identity recorded.
Changes since pre-GO: rubric 57 → 60 checks; applicable checks 1 to 28, 44 to 51, 54, 58 to 60; rulings R1 to R5 → R1 to R10. R10 records provisional answers to design § 8 questions, so do not mark those as defects. mock.js is a mockup harness, not shipped code; check 47(d) deferred to implementation. Candidate now named as 8f97e28; reference gaps above remain visible.
First action now: open the listed material via file:// in a browser at desktop and narrow widths; each mockup has a state switcher, light/dark toggle and intent-notes overlay and accepts ?state=…&theme=dark&annot=1. Apply the released rubric literally, with the named deferral.
Completion / review: deliver docs/wave-1/reviews/T2-gpt-review.md ≤120 lines, `git add` then pathspec commit on master; `RESULT T9 <commit>`, then `mesh task ruling 2 --kind verdict --value accepted|rejected --ref 8f97e28`. Keep the pre-GO findings, open questions, score table and confidence/uncertainty requirements. T2 acceptance owner product-reviewer reviews in parallel; read neither its findings nor anyone's opinion before committing your verdict. Stop at verdict committed; no design repair or invented criteria. BLOCKED T9 <reason> remains the pre-GO alternative if the contract cannot be satisfied.
Other waits: no other wait is named in this GO message; lifecycle absence/release of other waits is unverified, not asserted none.
```

### What the lead stops doing

**[I — UNVERIFIED design; basis: lead role `src-tauri/resources/templates/roles/v3-lead-claude.yaml:22`, `:205`; standard:27, :31]** Replace assignment-authoring guidance with four rules: (1) create the task with its five-line contract and any declared constraints/references; use the returned ID, (2) assign it once and let the record generate the notice, (3) record corrections and releases against the current generation and deliver their generated deltas, (4) use `mesh task create --help` for the available input surface. That help command was executed in probe 2; it does **not yet offer a review-route or structured-context input**. This document proposes their record shape, not an already-supported new flag. The role-text change is a handful of rules; it must replace equivalent compact-summary instructions too.

**[S] The current message lint checks task ID, first step, deliverable and completion signal by substring** (`/home/mstie/projects/mesh/src/main.rs:689`). Probe 27 sent a message lacking deliverable and exited 0 with this stderr: `[mesh] warning: actionable message missing required fields: deliverable` (`mesh-probes.log:434`). **[I — UNVERIFIED design]** Make the assignment-specific `deliverable:`-in-message check **obsolete**, not inverted into a ban on the word. Validate the record's contract at assignment. A legitimate GO/correction may mention a deliverable without becoming a duplicate assignment, and generic actionable messages still need clear routing/action/response semantics. Keep legacy lint during compatibility rollout; retire this full-contract demand only for identified generated assignment/release messages. Do not classify solely by a text prefix.

## Evidence

### Reproduced population and source index

**[P/S] The prior lane copied the threads study recipe through its task-population assertion** from `docs/design/mesh-task-threads-research.md:269` into `rebuild-index.py:1`; only the archive path was made absolute and output serialization added. Enumeration uses tar member order and **zero-based array index**, not timestamps. No filtering or sorting changes the pair index. The initial output serializer rejected a datetime; adding JSON `default=str` fixed only artifact serialization. The prior reconstruction reports exit 0; round 1 also reran `rebuild-index.py` successfully (`rebuild-index.log:23`) and independently checks its pinned archive source and pair assertions in `verify-report.py:1`. Inbox arrays and the three journals were also extracted as regular files into the scratch temporary directory recorded in `archive-extract-root.txt`; no live roots were used.

**[S] Real reconstruction output** (`rebuild-index.log:23`):

```text
assignment prose/card characters 15616 7485
assertions passed: 15616/7485; 62 task bodies; 84524 task characters
archive_sha256 60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1
```

**[S] The precise index is:** architect `(3,6)`; heavy-implementer `(3,5),(8,7),(11,12)`; heavy-implementer-1 `(3,5),(8,7)`; implementer-1 `(3,6),(10,9)`; judge-astra-1 `(3,2)`. Each tuple is authored/card, not chronological order (`rebuild-index.py:65`). The archive has 12 assignment events and one supersession; T9 changes from heavy-implementer-1 token `a387dac0-eae6-43e6-bb04-3d80880a36c6` to judge-astra-1 token `49f0e0e3-6833-40f8-906a-78419909a5b1` (`archive-task-inventory.txt:23`, `:32`). Nine pairs therefore span eight task IDs; counting task IDs loses a generation.

**[S] Snapshot inventory:** all eleven adjacent task snapshots have assignment ID/time/actor, first step, deliverable, completion signal, work kind, lane and criticality; none has `effort`, `effort_why` or `awaiting_go` (`archive-task-inventory.txt:1` through `:22`, one path and metadata row per task). The archive's task 9 snapshot is the final owner, not the first assignment. The workflow `task_assigned` records preserve the earlier three-field contract (`archive-task-inventory.txt:32`); the supersession record explains the owner change (`:34`). No full objective/review/immutable reference snapshot is invented from these events. These inventory statements were rechecked against the extracted archive event/row JSON and task-key inventory in round 1; the field assertions are reproducible via `verify-report.py:1`.

### Pair-by-pair field inventory

**[S] Common card fields in every row below:** subject and accountable owner in the header; task ID; full assignment token; actionable heading; first step; deliverable; completion signal; no-acknowledgment rule. Every authored message has all five numbered lines. “Both” below means semantic overlap, not text equality; detailed content remains in the cards re-rendered below and the exact source transcription `nine-pairs-source.txt`. The final column names information lost by choosing only the prose or only the card.

| Pair / evidence | Authored fields and context [S] | Card fields beyond the common set / overlap [S] | One-sided fields or conflicts [S] |
|---|---|---|---|
| 1 T1, architect 3/6; source:1 | Five lines: reserved decisions; architecture path and seven sections; two-document read; RESULT/BLOCKED plus incoming T3/4/5 inputs; spec-delta/lead/altitude route. Repo, standard, ledger, safety and commit constraints. | Same first read/path/results/acceptance and altitude reviewer. | Prose only: section requirements, rejected alternatives, architecture/UI seam, input handling, no input copying. Card only: machine token/no-ack plus **master**, versus prose **main**. |
| 2 T4, heavy 3/5; source:23 | Five lines: five-area prior-art map; per-area pointers/mechanism/tests/verdict, model-effort flag spellings; search/read; RESULT/BLOCKED; diagnose/architect. Repo and commit constraints. | Same search/read/path/verdicts/architect. | Prose only: required inventory detail, no product code/no taurhaus changes. Card adds token/no-ack and master versus main. |
| 3 T8, heavy 8/7; source:45 | Five lines: fixed-stack readiness; tool versions, running scratch build, time/failures, ≤80 added lines; versions then house recipe; RESULT/BLOCKED; measure/architect→T1. T4 received hash/count and conditional closeout. | Same stack/path/scratch/build-command result; card puts WebKitGTK/system dependency check explicitly in first step. | Prose only: fixed stack includes Rust, no npm/npx, wall time/failure details, prior-task obligation; card ≤80 lines versus prose ≤80 **added** lines. Retain counting ambiguity. |
| 4 T11, heavy 11/12; source:65 | Five lines: buildable M0; enumerated app/IPC/gate/locks, 400-line exclusions, forbidden paths; toolchain/architecture/scratch merge/red skip; gate+build RESULT/BLOCKED; lead/altitude/M1 seam. Prior T8 closeout and operator approval. | Same files/stack/gate/budget/action/result/reviewers. | Prose only: no samples/mocks, typed not-implemented errors, forward-only migrations, forbidden files, generated-file list, red skip, cross-family explanation, M0/M1 boundary and previous closeout. Card says cargo clippy -D warnings explicitly. |
| 5 T5, heavy-1 3/5; source:85 | Five lines: packaging inventory; eight stages and detailed schemas/redacted rows; SPEC/runners read and personal-data restrictions; stage verdict RESULT/BLOCKED; diagnose/architect. Repo/commit constraints. | Same first documents/path/stage-verdict/architect. | Prose only: detailed input/output/duration/failure inventory, read-only/no personal material, schema examples. Card adds token/no-ack and master versus main. |
| 6 T9 generation 1, heavy-1 8/7; source:107 | Five lines: GPT-on-Claude review; 120-line findings/questions/table/verdict; pre-read and wait; RESULT plus candidate-referenced verdict/BLOCKED; product-reviewer acceptance, independent hero-surface reviewer. T5 received hash/count and conditional closeout. | Same pre-read/release phrase/path/result; card includes ruling operation but omits candidate ref. | Prose only: candidate-ref requirement, findings schema, independent-verdict isolation, hero surfaces, previous closeout. Card heading says start now although its action says wait. |
| 7 T3, implementer 3/6; source:127 | Five lines: real per-CLI streams; raw files/README per-CLI command/exit/timing/events/final/errors and facts update; Claude then Codex/agy/Grok sequence, no personal prompt data; per-CLI RESULT/BLOCKED; measure/architect. Repo/commit constraints. | Same Claude first command, sample path/facts update, result/architect. | Prose only: full CLI sequence and evidence contract, safety/no product/scaffold. Card adds token/no-ack and master versus main. |
| 8 T10, implementer 10/9; source:149 | Five lines: HTTP parity number; named portals, endpoints/verification/text/failures/coverage, 120 lines; inventory+architecture/public probes, bounded fetch/no candidate/CLI/product; RESULT/BLOCKED; measure/architect with lead decision and future M2 seam. | Same references/probe/public-only/path/results/lead decision; card says each listed portal type. | Prose only: portal names and browser-only list, bounded-fetch and prohibited material/code, facts not recommendation, future M2 scope. |
| 9 T9 generation 2, judge 3/2; source:169 | Five lines: buildability/product gate; findings/questions/table/confidence/verdict; 57-check rubric subset, R1–R5, site, wait then browser widths; RESULT+candidate-linked verdict/BLOCKED; product acceptance/cross-family/isolation/stopping. Repo/standard/no remote/no bake-off. | Same pre-read/rubric count/path/pathspec/result/candidate ref. | Prose only: applicable check numbers, confidence record, candidate artifact paths and widths, all-member-opinion isolation, stopping/no repair/no invented criteria, repo context. Card start-now heading conflicts with wait. Neither freezes candidate/rubric hash at assignment. |

### Nine combined cards, reconstructed from both sources

**[I — UNVERIFIED design]** Literal-template retrospective renderings below are not delivered messages or authority to execute archived commands. Five-line values and introductory constraints are copied from the authored sources; fifth-line checkout/isolation tails move verbatim into Context (standard:25, :90). No command is redacted or paraphrased. Reconciliation is an explicit optional template slot. State reports the assignment event, not the final snapshot; an unavailable marker is not an inferred hold or release. A conflicting field creates the visible BLOCKED route, not a lifecycle mutation by the renderer. The historical pre-read instruction remains binding even where the archive lacks its matching machine wait. Source-to-render assertions and their limits are in `verify-report.py:1`.

**[S]** The nine pairs supply no effort/reason/deadline overrides. Real line ceilings appear in Budget; missing counting tools and frozen references remain marked unavailable. Their later T9 GO is rendered separately below (`nine-pairs-source.txt:107`, `:169`; `t9-go-evidence.txt:1`).

#### Pair 1: task #1, architect, authored 3 / card 6

**[S] Source:** `nine-pairs-source.txt:1`; prose 2,200 characters, card 638. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#1 T1: Wave-1 architecture decision packet · owner: architect · team: taurjob-team]
Assignment: a42960aa-ca81-4339-8a24-def1a09aa80f · issued by lead-taurjob at 2026-09-06T19:42:22.271Z
ACTION REQUIRED: Start permitted action: read BRIEF.md, then job-hunt/SPEC.md.
Execution: RECORDED status=in_progress; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: decide the reserved decisions in BRIEF.md § "Decisions reserved for the team" so implementation can start with one accountable owner per surface.
Deliverable: `~/projects/taurjobs/docs/wave-1/architecture.md`, committed on main. Sections: (a) process supervision and progress architecture per CLI, grounded in `docs/reference/headless-cli-facts.md`; (b) run/persistence model and storage schema, including seen-state migration from `seen.md` and `state/*.jsonl`; (c) how the existing pipeline prompts/stages are packaged and invoked; (d) CLI and model discovery plus model-per-task config shape; (e) milestone slicing within wave 1 (configure → run once with live progress → browse results) with one implementer and one acceptance owner named per surface; (f) the per-task gate command the repo will use (CLAUDE.md names none yet); (g) a 5-line repo scaffold recipe for the fixed stack (Tauri 2 + Rust + Svelte 5 + Tailwind v4, bun only). Each decision states the alternative rejected and why in one or two sentences. Stack challenge only with a written concrete reason.
First action: read `~/projects/taurjobs/BRIEF.md`, then `~/projects/job-hunt/SPEC.md` in full (read-only repo; never copy `inputs/` content anywhere).
Completion signal: message lead-taurjob `RESULT T1 <commit hash>` with the decision list as bullets, or `BLOCKED T1 <reason>`. Inputs from T3 (real CLI stream samples), T4 (taurhaus prior-art pointers), T5 (pipeline stage inventory) will be routed to you as they land; do not wait for them to start, revise when they arrive.
Review route: spec-delta; acceptance owner lead-taurjob; altitude-reviewer does a Claude altitude pass after RESULT. Seam: you decide architecture; implementers decide in-scope details; the design packet (T2, design-taurjob) owns UI/UX and you consume its surface list, not the reverse.
Context: ACTION REQUIRED: T1 — write the wave-1 architecture decision packet for taurjobs. | Working repo: `~/projects/taurjobs` (not the empty `~/projects/taurjob` cwd). Standard: `~/projects/taurhaus/docs/team-delivery-standard.md`. Ledger: `~/projects/taurjobs/docs/wave-1/ledger.md`. | Commit only your file; `git pull --rebase` before committing.
Reconciliation: CONFLICT: authored deliverable says main; recorded deliverable says master. BLOCKED route: assignee records task #1 blocked through the existing task block lifecycle and sends BLOCKED T1 <reason> to lead-taurjob, naming assignment a42960aa-ca81-4339-8a24-def1a09aa80f, artifact=branch ruling, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 2: task #4, heavy-implementer, authored 3 / card 5

**[S] Source:** `nine-pairs-source.txt:23`; prose 1,466 characters, card 656. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#4 T4: Taurhaus prior-art inventory for borrowable patterns · owner: heavy-implementer · team: taurjob-team]
Assignment: b7def64e-5c01-4b66-9e9e-1ace6db9645d · issued by lead-taurjob at 2026-09-06T19:42:22.624Z
ACTION REQUIRED: Start permitted action: search taurhaus/src-tauri/src for process spawning and JSONL parsing.
Execution: RECORDED status=in_progress; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: a factual map of taurhaus code for CLI detection, process supervision, streaming JSONL parsing, SQLite persistence, and Tauri command layout, so T1 borrows patterns deliberately instead of cargo-culting or rediscovering them.
Deliverable: `~/projects/taurjobs/docs/reference/taurhaus-prior-art.md` committed on main. For each of the five areas: the file:line pointers, a three-sentence description of the mechanism, its tests, and a verdict of borrow as-is / adapt / do not borrow with one sentence of reason. Also record the model and effort flag spellings in `~/projects/taurhaus/src-tauri/src/session_scanner/cli_tool.rs` since model-per-task must mirror them. Diagnose kind: read-only, no product code.
First action: `grep -rn` in `~/projects/taurhaus/src-tauri/src` for process spawning and JSONL parsing entry points, then read `~/projects/taurhaus/CLAUDE.md` for its build and test recipes.
Completion signal: message lead-taurjob `RESULT T4 <commit hash>` with the five verdicts as bullets, or `BLOCKED T4 <reason>`.
Review route: diagnose; acceptance owner architect.
Context: ACTION REQUIRED: T4 — inventory the taurhaus prior art the architect may borrow. | Working repo: `~/projects/taurjobs` (not the empty `~/projects/taurjob` cwd). Standard: `~/projects/taurhaus/docs/team-delivery-standard.md`. Ledger: `~/projects/taurjobs/docs/wave-1/ledger.md`. | Commit only your file; `git pull --rebase` before committing. Do not modify taurhaus.
Reconciliation: CONFLICT: authored deliverable says main; recorded deliverable says master. BLOCKED route: assignee records task #4 blocked through the existing task block lifecycle and sends BLOCKED T4 <reason> to lead-taurjob, naming assignment b7def64e-5c01-4b66-9e9e-1ace6db9645d, artifact=branch ruling, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 3: task #8, heavy-implementer, authored 8 / card 7

**[S] Source:** `nine-pairs-source.txt:45`; prose 1,347 characters, card 833. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#8 T8: Toolchain readiness for Tauri 2 + Svelte 5 + Tailwind v4 (bun-only) · owner: heavy-implementer · team: taurjob-team]
Assignment: 1665fd25-9329-44b7-a8cb-3cfcf1323585 · issued by lead-taurjob at 2026-09-06T19:45:41.420Z
ACTION REQUIRED: Start permitted action: run the tool-version checks in First action.
Execution: RECORDED status=pending; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: establish on this machine that the fixed stack (Tauri 2 + Rust + Svelte 5 + Tailwind v4, bun only, never npm or npx) scaffolds and dev-builds, so the first implement task starts from verified commands.
Deliverable: `~/projects/taurjobs/docs/reference/toolchain.md`, ≤ 80 added lines, committed on master with an explicit pathspec. Contents: version per tool (bun, rustc, cargo, tauri CLI, WebKitGTK and other deps that `tauri info` reports), the exact command sequence that produced a running dev build in a `/tmp` scratch directory, dev-build wall time, and any failure verbatim. The scratch scaffold is never committed anywhere.
First action: run `bun --version`, `rustc --version`, `cargo --version`, `bunx @tauri-apps/cli --version`, then read `~/projects/taurhaus/CLAUDE.md` for the house build recipe to mirror.
Completion signal: `RESULT T8 <commit>` with per-tool status and the verified command list, or `BLOCKED T8 <reason>`.
Review route: measure kind; acceptance owner architect, who consumes it into the T1 scaffold recipe. No product code in the repo under this task.
Budget: baseline not recorded; paths docs/reference/toolchain.md; ceiling authored ≤80 added lines / card ≤80 lines; counting rule unresolved; tool and exclusions not recorded; scratch scaffold never committed.
Context: ACTION REQUIRED: T4 received at c563ff3 and lead-verified (169/180 lines, one file); the architect owes the acceptance ruling on #4, after which run `mesh task complete 4`. Your next assignment is mesh task #8 (T8), start now in parallel. | Recorded first-action addition: check WebKitGTK/system dependencies reported by tauri info.
Reconciliation: CONFLICT: authored budget says ≤80 added lines; recorded deliverable says ≤80 lines. No counting rule selected by renderer. BLOCKED route: assignee records task #8 blocked through the existing task block lifecycle and sends BLOCKED T8 <reason> to lead-taurjob, naming assignment 1665fd25-9329-44b7-a8cb-3cfcf1323585, artifact=budget counting ruling, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 4: task #11, heavy-implementer, authored 11 / card 12

**[S] Source:** `nine-pairs-source.txt:65`; prose 1,997 characters, card 1,216. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#11 T11 / M0: app scaffold, per-task gate, IPC type baseline · owner: heavy-implementer · team: taurjob-team]
Assignment: fd8716fc-8e3f-4e94-b6a7-82fd5c89e159 · issued by lead-taurjob at 2026-09-06T19:55:59.514Z
ACTION REQUIRED: Start permitted action: read docs/reference/toolchain.md and architecture sections 6–7.
Execution: RECORDED status=pending; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: a committed, buildable app shell on the fixed stack with the per-task gate working, so M1 to M3 start from one baseline instead of three.
Deliverable: on master in `~/projects/taurjobs`: `src/` with a Svelte 5 runes-only empty shell and Tailwind v4 `@theme` semantic tokens (no sample components, no mock data); `src-tauri/` with Tauri 2, minimal capabilities, rusqlite dependency, a forward-only numbered migration runner with an empty v1; `src/lib/ipc.ts` typed wrapper baseline with the command names from architecture § 6 as stubs that return a typed not-implemented error; `package.json` script `check:task` running svelte-check, vitest run, `cargo fmt --check`, `cargo clippy` with warnings denied, and `cargo test --manifest-path src-tauri/Cargo.toml`; `bun.lock` and `Cargo.lock`. Never touch BRIEF.md, CLAUDE.md, docs/, or the nested `.git`. Budget: ≤ 400 hand-written lines excluding generated and lock files; list the generated files in the RESULT.
First action: read `docs/reference/toolchain.md` and architecture § 6 and § 7, run the plain Svelte/Vite plus `tauri init --ci` sequence exactly as toolchain.md records in a /tmp sibling, then merge only generated app files into the repo. Implement kind, scaffolding: red-first skipped, say so in the RESULT.
Completion signal: `RESULT T11 <commit>` with the tail of `bun run check:task` and the result of `bunx tauri build --debug --no-bundle`, or `BLOCKED T11 <reason>`.
Review route: acceptance owner lead-taurjob; structural review by altitude-reviewer (Claude family, cross-family on your Astra code). Seam: after M0 you continue into M1 (configure, settings, db, commands); the schema v1 tables from architecture § 3 belong to M1, not M0.
Budget: baseline not recorded; paths src/, src-tauri/, src/lib/ipc.ts, package.json; ceiling ≤400 hand-written lines; generated and lock files excluded; list generated files in RESULT; counting tool not recorded.
Context: ACTION REQUIRED: T8 received at bfc2cca and lead-verified (77/80 lines); the architect owes its acceptance ruling, then `mesh task complete 8`. The operator approved the architecture packet, so M0 starts now as mesh task #11. Start in parallel with the #8 closeout. | Record explicitly specifies cargo clippy -D warnings.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 5: task #5, heavy-implementer-1, authored 3 / card 5

**[S] Source:** `nine-pairs-source.txt:85`; prose 1,633 characters, card 615. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#5 T5: Job-hunt pipeline stage inventory · owner: heavy-implementer-1 · team: taurjob-team]
Assignment: c1bb2a6a-b6f4-40ef-a51e-bf98c0942496 · issued by lead-taurjob at 2026-09-06T19:42:22.748Z
ACTION REQUIRED: Start permitted action: read job-hunt/SPEC.md sections 2–9, then pipeline/jh/runners.py.
Execution: RECORDED status=in_progress; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: a factual, per-stage contract of the existing pipeline so the architect can decide what is invoked verbatim, what is restructured, and what Python survives.
Deliverable: `~/projects/taurjobs/docs/reference/pipeline-stages.md` committed on main. For each of the eight stages in SPEC § 2: inputs (which documents, which prior-stage outputs), the prompt file in `pipeline/prompts/`, the runner and exact CLI invocation in `pipeline/jh/runners.py`, the output file and its parsed shape, expected duration, and failure modes noted in SPEC or HANDOVER. Plus the schemas of `state/roles.jsonl`, `state/runs.jsonl`, and the `seen.md` table, with one redacted example row each. Diagnose kind: read-only, no product code.
First action: read `~/projects/job-hunt/SPEC.md` § 2 through § 9, then `~/projects/job-hunt/pipeline/jh/runners.py`. `~/projects/job-hunt` is read-only reference; never modify it, and never copy anything from `inputs/` or personal data from `seen.md`, `runs/`, or `opportunities.md` into your deliverable.
Completion signal: message lead-taurjob `RESULT T5 <commit hash>` with a one-line verdict per stage on whether it can run unchanged from a Rust supervisor, or `BLOCKED T5 <reason>`.
Review route: diagnose; acceptance owner architect.
Context: ACTION REQUIRED: T5 — inventory the job-hunt pipeline stage by stage for packaging into the app. | Working repo: `~/projects/taurjobs` (not the empty `~/projects/taurjob` cwd). Standard: `~/projects/taurhaus/docs/team-delivery-standard.md`. Ledger: `~/projects/taurjobs/docs/wave-1/ledger.md`. | Commit only your file; `git pull --rebase` before committing.
Reconciliation: CONFLICT: authored deliverable says main; recorded deliverable says master. BLOCKED route: assignee records task #5 blocked through the existing task block lifecycle and sends BLOCKED T5 <reason> to lead-taurjob, naming assignment c1bb2a6a-b6f4-40ef-a51e-bf98c0942496, artifact=branch ruling, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 6: task #9, heavy-implementer-1, authored 8 / card 7

**[S] Source:** `nine-pairs-source.txt:107`; prose 1,481 characters, card 862. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#9 T9: GPT-family review of T2 design packet · owner: heavy-implementer-1 · team: taurjob-team]
Assignment: a387dac0-eae6-43e6-bb04-3d80880a36c6 · issued by lead-taurjob at 2026-09-06T19:46:35.786Z
ACTION REQUIRED: Pre-read only; execution waits for the authored T9 GO.
Execution: RECORDED status=pending; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: the GPT-family review of the Claude-authored T2 design packet, so the two-family route holds on a hero-surface design.
Deliverable: `~/projects/taurjobs/docs/wave-1/reviews/T2-gpt-review.md`, ≤ 120 lines, committed on master with an explicit pathspec. Numbered findings in severity order with evidence, impact, and action; open questions separate from defects; score table Finding / Severity / Confidence / Action; verdict line accept, bounded correction, or reject.
First action: pre-read `BRIEF.md`, `docs/wave-1/acceptance.md` sections 1 to 3 and 6 to 7, job-hunt `SPEC.md` § 0, and the rendered page under `~/projects/job-hunt/site/` now. I will send `T9 go <commit>` when design-taurjob reports T2.
Completion signal: `RESULT T9 <commit>` with verdict line and finding count, then `mesh task ruling 2 --kind verdict --value accepted|rejected --ref <T2 commit>`; or `BLOCKED T9 <reason>`.
Review route: review kind, cross-family. Acceptance owner for T2 stays product-reviewer; you are the second, independent reviewer because run-progress and results browser are declared hero surfaces.
References: candidate pending T2 delivery; rubric named in First action; immutable candidate/rubric revisions and packet digest not recorded at pre-GO.
Budget: evidence deliverable ≤120 lines; baseline/counting tool not recorded; measure/review evidence contract, not an inferred implementation diff leash.
Context: ACTION REQUIRED: T5 received at 79d3657 and lead-verified (304/350 lines, one file, no personal strings); the architect owes the acceptance ruling on #5, after which run `mesh task complete 5`. Your next assignment is mesh task #9 (T9); pre-read starts now, the review itself waits for my go. | Do not read product-reviewer's T2 findings before recording your own.
Reconciliation: CONFLICT: recorded start-now heading contradicts recorded first_step and authored pre-read-only instruction; no assignment-time UUID wait marker is evidenced. Preserve the pre-read instruction; missing lifecycle declaration needs repair. BLOCKED route: assignee records task #9 blocked through the existing task block lifecycle and sends BLOCKED T9 <reason> to lead-taurjob, naming assignment a387dac0-eae6-43e6-bb04-3d80880a36c6, artifact=T2 candidate and recorded GO, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 7: task #3, implementer-1, authored 3 / card 6

**[S] Source:** `nine-pairs-source.txt:127`; prose 1,744 characters, card 692. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#3 T3: Capture real headless stream samples per CLI · owner: implementer-1 · team: taurjob-team]
Assignment: 0a285384-f5e3-42f8-a2ef-dbd47deeb000 · issued by lead-taurjob at 2026-09-06T19:42:22.506Z
ACTION REQUIRED: Start permitted action: capture the Claude stream with the exact command in First action.
Execution: RECORDED status=in_progress; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: give the architect and the supervisor implementer verified, on-disk event streams per CLI so the progress architecture is designed and tested against real output, not the docs.
Deliverable: `~/projects/taurjobs/docs/reference/cli-stream-samples/<cli>.jsonl` (raw stdout, one file per CLI that works) plus `docs/reference/cli-stream-samples/README.md` recording for each CLI: exact command run, exit code, whether output arrived incrementally (timestamp the first and last line), the event types observed, and how the final answer and any error are represented. Update `docs/reference/headless-cli-facts.md` where it says unverified and you now have evidence. Committed on main.
First action: run `claude -p "Reply with the single word ready" --output-format stream-json --include-partial-messages --verbose` with stdout teed to the sample file and a per-line timestamp, then the codex equivalent from the facts table (`codex exec --json ... </dev/null`), then probe agy and grok. Use a trivial prompt; never include CV, reference, or any personal document. Measure kind: no product code, no scaffolding.
Completion signal: message lead-taurjob `RESULT T3 <commit hash>` with a one-line verdict per CLI (streams / whole-response / not installed / not logged in), or `BLOCKED T3 <reason>`.
Review route: measure; acceptance owner architect (they consume the samples in T1).
Context: ACTION REQUIRED: T3 — capture real headless streaming samples from the installed CLIs. | Working repo: `~/projects/taurjobs` (not the empty `~/projects/taurjob` cwd). Standard: `~/projects/taurhaus/docs/team-delivery-standard.md`. Ledger: `~/projects/taurjobs/docs/wave-1/ledger.md`. | Commit only your files; `git pull --rebase` before committing.
Reconciliation: CONFLICT: authored deliverable says main; recorded deliverable says master. BLOCKED route: assignee records task #3 blocked through the existing task block lifecycle and sends BLOCKED T3 <reason> to lead-taurjob, naming assignment 0a285384-f5e3-42f8-a2ef-dbd47deeb000, artifact=branch ruling, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 8: task #10, implementer-1, authored 10 / card 9

**[S] Source:** `nine-pairs-source.txt:149`; prose 1,534 characters, card 1,130. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#10 T10: Portal coverage parity measure for Rust-side ATS verification · owner: implementer-1 · team: taurjob-team]
Assignment: 402eff75-ceeb-486a-a500-e4088eeaf42d · issued by lead-taurjob at 2026-09-06T19:53:13.604Z
ACTION REQUIRED: Start permitted action: read pipeline-stages.md and architecture section 4, then probe public portals.
Execution: RECORDED status=pending; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: a number the lead can decide on: how much of the job-hunt pipeline's posting verification and text capture survives a pure HTTP(S) port, and which portal types are lost.
Deliverable: `~/projects/taurjobs/docs/reference/portal-coverage.md`, ≤ 120 lines, `git add` then pathspec commit on master. Per portal type from T5's inventory (Ashby, Greenhouse, SmartRecruiters, Personio, Workday, Lever, plus the browser-only ones T5 lists): endpoint used, whether verification and posting-text capture succeed with plain requests, failure text verbatim, and a summary line of recovered vs lost coverage against the existing pipeline.
First action: read `docs/reference/pipeline-stages.md` for the browser-only portal list and `docs/wave-1/architecture.md` § 4 for the ported verification design, then probe with curl or a short script in /tmp against public postings only. No candidate documents, no CLI agents, no product code, bounded fetch sizes.
Completion signal: `RESULT T10 <commit>` with the recovered/lost summary, or `BLOCKED T10 <reason>`.
Review route: measure kind; acceptance owner architect. Report facts, not a recommendation; the lead decides on a bounded browser helper from your number. You remain the named M2 implementer (run lifecycle, CLI adapters, run-progress surface) once the packet is approved.
Budget: evidence deliverable ≤120 lines; baseline/counting tool not recorded; measure/review evidence contract, not an inferred implementation diff leash.
Context: ACTION REQUIRED: T10 (mesh task #10) — measure whether Rust-side plain HTTP verification recovers the existing pipeline's portal coverage. This is the gate the architecture packet left unowned. | Recorded first-action scope: each listed public portal type, including browser-only types in the inventory.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

#### Pair 9: task #9, judge-astra-1, authored 3 / card 2

**[S] Source:** `nine-pairs-source.txt:169`; prose 2,214 characters, card 843. **[I — UNVERIFIED design]** Reconciliation routes are proposed obligations; execution fields report the source event.

```text
[#9 T9: GPT-family review of T2 design packet · owner: judge-astra-1 · team: taurjob-team]
Assignment: 49f0e0e3-6833-40f8-906a-78419909a5b1 · issued by lead-taurjob at 2026-09-06T19:51:20.081Z
ACTION REQUIRED: Pre-read only; execution waits for the authored T9 GO.
Execution: RECORDED status=pending; assignment-time awaiting_go value unavailable in this event; no lifecycle transition inferred.
Objective: judge whether design-taurjob's packet (configure, run-progress, results browser) satisfies the acceptance gate and the inherited product principles, and whether it is buildable as specified.
Deliverable: `~/projects/taurjobs/docs/wave-1/reviews/T2-gpt-review.md`, ≤ 120 lines, committed on master. Numbered findings in severity order, each citing the mockup file or design.md line as evidence with impact and action; open questions separate from defects; score table Finding / Severity / Confidence / Action; a confidence and uncertainty record; verdict line accept, bounded correction, or reject.
First action: read `docs/wave-1/acceptance.md` (57 checks; that is your rubric, applied literally, checks 1 to 28, 44 to 51, and 54 apply), `docs/wave-1/rulings.md` R1 to R5, `BRIEF.md`, job-hunt `SPEC.md` § 0, and the rendered reference page under `~/projects/job-hunt/site/`. Material arrives as `T9 go <commit>`: `docs/wave-1/design.md` plus `docs/wave-1/mockups/*.html`, which you open in a browser at desktop and narrow widths.
Completion signal: `RESULT T9 <commit>` with verdict line and finding count, then `mesh task ruling 2 --kind verdict --value accepted|rejected --ref <T2 commit>`; or `BLOCKED T9 <reason>`.
Review route: review kind, cross-family (Astra on Claude-authored design). Acceptance owner for T2 is product-reviewer, who reviews independently.
References: candidate pending T2 delivery; rubric named in First action; immutable candidate/rubric revisions and packet digest not recorded at pre-GO.
Budget: evidence deliverable ≤120 lines; baseline/counting tool not recorded; measure/review evidence contract, not an inferred implementation diff leash.
Context: ACTION REQUIRED: T9 (mesh task #9) — the GPT-family review of the wave-1 design packet, single-sided visual review under verdict isolation. Pre-read now; the review itself starts on my go. | Working repo: `~/projects/taurjobs` on `master` (not the empty `~/projects/taurjob` cwd; no remote; pathspec commits only). Standard: `~/projects/taurhaus/docs/team-delivery-standard.md` (canonical for this wave). Repo instructions: `~/projects/taurjobs/CLAUDE.md`, no per-task gate yet because there is no code. No bake-off runs this wave, so there is no second judge; you review the artifact alone. | Isolation: do not read product-reviewer's T2 findings or any member's opinion of the packet before your verdict is committed. Stopping condition: verdict committed; no repair of the design, no invented criteria beyond the rubric.
Change: supersedes a387dac0-eae6-43e6-bb04-3d80880a36c6 because recorded workflow reason=reassigned; admin reason not recorded; changed owner heavy-implementer-1 → judge-astra-1 and authored first action/review scope.
Reconciliation: CONFLICT: recorded start-now heading contradicts recorded first_step and authored pre-read-only instruction; no assignment-time UUID wait marker is evidenced. Preserve the pre-read instruction; missing lifecycle declaration needs repair. BLOCKED route: assignee records task #9 blocked through the existing task block lifecycle and sends BLOCKED T9 <reason> to lead-taurjob, naming assignment 49f0e0e3-6833-40f8-906a-78419909a5b1, artifact=T2 candidate and recorded GO, owner=lead-taurjob, release=authoritative correction for this generation. This annotation is an obligation, not a recorded block or a new hold state.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

### Synthetic card: optional sections exercised

**[I — UNVERIFIED design]** Synthetic identifiers, references, clock and wait are fixture input, not real commits, tests, or approvals (`synthetic-rendered-card.json:1`). This uses the same renderer as all nine historical cards.

```text
[#synthetic-1 Illustrative frozen review · owner: fixture-reviewer · team: fixture-only]
Assignment: 22222222-2222-4222-8222-222222222222 · issued by fixture-lead at 2026-09-08T12:00:00Z
Effort: high — synthetic isolation-sensitive review
ACTION REQUIRED: Pre-read only; execution waits for fixture-lead release.
Execution: WAIT awaiting_go=22222222-2222-4222-8222-222222222222 matches assignment; owner fixture-lead; artifact fixture-candidate; since 2026-09-08T12:00:00Z, age 0 at issuance; release fixture manifest verified.
Objective: Review the synthetic fixture against its frozen rubric.
Deliverable: fixture-result.md with findings and score table.
First action: Read fixture-rubric.md now; inspect fixture-candidate only after GO.
Completion signal: RESULT with evidence or BLOCKED with task/generation/artifact/owner/release condition.
Review route: review; acceptance fixture-lead; reviewer fixture-reviewer; handoff fixture-result.md.
References: accepted base fixture-base object aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; candidate pending; rubric fixture-rubric.md sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb; packet fixture-packet sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
Deadline: 30 minutes
Budget: baseline fixture-base; paths fixture-result.md; counting final file lines; tool fixture-count.py; exclusions none; ceiling 120.
Context: checkout fixture-only; manifest stopping=verdict committed; allowed evidence=own candidate/rubric only; no peer opinions.
Change: supersedes 11111111-1111-4111-8111-111111111111 because synthetic admin reason=replace reviewer; changed owner, candidate availability and wait.
No pure acknowledgment. Follow the completion signal; report an execution blocker as specified.
```

### Compatibility and inspection surfaces

**[S] Current spellings and consumers:** Mesh's contract resolver reads snake_case and camelCase aliases for first step/completion (`/home/mstie/projects/mesh/src/main.rs:310`); assignment writes the snake_case keys (`/home/mstie/projects/mesh/src/task_lifecycle.rs:260`). Taurhaus's task scanner reads `effort`, `effort_why`/`effortWhy`, and `deadline_minutes`, but its `UnifiedTask` construction does not carry the complete five-line contract or raw metadata (`src-tauri/src/task_scanner/claude.rs:491`, `:507`; `src-tauri/src/task_scanner/types.rs:74`). Renaming or nesting the existing effort/deadline fields would break those reads. A task with source `work_kind=docs` is not thereby semantically “spec-delta”; the wave-1 T1 snapshot supplies a concrete example (`archive-task-inventory.txt:2`).

**[S] A wait compatibility mismatch is visible in source and the isolated wire observation.** Mesh sets `awaiting_go` to the current assignment UUID (probe 10, `mesh-probes.log:205`), and checks equality to that UUID (`/home/mstie/projects/mesh/src/idle_monitor.rs:714`). Taurhaus's `coordination/stores/mesh_task.rs:17` checks only `.as_bool() == Some(true)`; its status-write guard and open-task probe consume that function at `:52` and `:106`. **[S] Contract-repair lane item:** `src-tauri/src/coordination/stores/mesh_task.rs:233` also writes boolean `awaiting_go = true` in its deadline regression fixture; that fixture must move to the canonical UUID contract in the repair lane, alongside the reader, not in this rendering-only revision. **[U — UNVERIFIED]** No running daemon was tested, so this does not establish an observed production nudge or status error. A paired isolated reader test using the exact UUID-valued record would settle the runtime consequence. Safe default: do not declare end-to-end wait preservation until the phase-0 repair/paired review reconciles this; do not change the wire key to boolean to make one consumer happy.

| Surface | Existing behavior [S] | Required behavior [I — UNVERIFIED design] |
|---|---|---|
| Directed inbox notice | Assignment formats the committed task, then sends it; failures can leave assignment committed without notice (`main.rs:3523`, `:3539`). | One complete body per assignment-generation obligation; preserve existing recipient routing and failure reporting. Record existence must never count as delivered. No new “exactly once” promise. |
| Assign command output | Today successful assign stdout reports message ID, while task-get JSON reports assignment ID (round-1 probes 9/10, 17/18, 23/24; `main.rs:3561`). | Add separately labelled `assignment_id` and `message_id` plus delivery disposition to assign output; proposed assign JSON should return both without reinterpreting current `id`. Keep current output compatible until paired approval. Safe default today: second task-get JSON lookup, compare owner and task/generation, never parse the message ID as a token. |
| Human-readable task get | Shows subject, owner/token, three fields, effort/deadline; description truncates at 160 and metadata is summarized (`main.rs:1537`, `:1559`; probe 11). Wait appears only as a metadata key in this probe. | Same complete contract renderer plus lifecycle state; explicit operative wait and refs visible without a second lookup. No truncated required contract. |
| JSON task get | Probe 10 returns task plus `lifecycleStage`, `countsAsActiveLane`, `pendingEffort`, `deadlineMinutes`; full metadata preserved. | Preserve existing top-level names/types. Optional new metadata stays readable without changing old values. Current record is the authoritative second copy of the contract data, not separately authored text. |
| Taurhaus Scope | The threads study describes a bounded Scope projection and distinct read/source/event times (`docs/design/mesh-task-threads-research.md:240`). Scanner evidence above is narrower than a complete Scope implementation. | Use the same normalized contract values and reference/access policy; compact layout may fold optional detail but cannot discard it. A human read mark neither clears a wait nor proves agent exposure. **U:** actual future Scope byte/exposure behavior needs implementation and isolated view tests. |
| Restricted review | D: adjudication requires all projections to exclude peer verdicts and their derivatives for unlocked reviewers (`docs/design/mesh-next-adjudication.md:51`). | Filter before rendering every surface, including references that would reveal peer artifacts; return only authorized candidate/rubric/own evidence. No automatic opening of mutable paths or embedding another review's summary. |

**[I — UNVERIFIED design] Minimal compatibility envelope:** add `review_route` and optional `assignment_context` with the explicit lifetime/writer table; preserve all flat legacy fields and the existing message-ID meaning of assign stdout. Add a separately labelled assignment-token surface and proposed structured assign output only after paired approval. Add machine-maintained `previous_assignment_id`, `assignment_change_reason` and `changed_fields` inside `assignment_context` when reissuing, derived at the same mutation boundary as the existing supersession/admin facts. Legacy snake_case takes precedence when nonempty; camelCase remains a read fallback. If both spellings disagree, show an explicit conflict and require author resolution for a new actionable assignment; do not silently combine values. Keep incomplete historical records inspectable and label their coverage. Basis: current alias resolution (`main.rs:320`, `:345`) and correction precedence in the standard (`:27`).

**[S/D] Mesh's own version rule is narrow:** `/home/mstie/projects/mesh/src/version_info.rs:8` says stable required version-payload keys and additive-only changes within a compatibility line. It does not certify arbitrary new lifecycle semantics. The overhaul study explicitly permits optional defaulted read additions but requires paired contract review for changed command response meanings, runtime delivery ownership or incompatible vocabulary (`docs/design/mesh-messaging-overhaul-research.md:217`). **[I — UNVERIFIED design]** Optional record fields and an optional rendering are plausibly additive; stricter five-line assignment validation, retiring the old lint expectation and automatically emitting a GO release are behavioral changes. Submit those together to paired Mesh/Taurhaus contract review before changing the active pin. Keep protocol 1/schema 1/app-daemon 24 untouched in this design; do not invent a next version number or claim a bump is unnecessary. No new storage authority, daemon, transport, subscription or migration-on-read belongs in this lane.

### Executed command evidence

**[S] Round-1 reproduction:** `python3 .check-logs/mesh-next-phase0/round1-probe.py` exited 0; `round1-probe.log:1` and `mesh-probes.json:1` record 41 invocations. Each argv starts `/home/mstie/.local/bin/mesh --claude-dir <fresh tempdir> --team render-probe --name <synthetic actor>`, cwd is that fresh root, child env is `{}`, and each process is waited or killed/waited in `finally` (`probe-mesh.py:8`, `probe-more.py:4`). Exact full argv/output/exit are in `mesh-probes.log:1`; the shorthand below omits only those recorded global arguments. No archived command, real model CLI, tmux, daemon or gate was run. Authentication credentials are never printed. Assign stdout IDs are message IDs; task-get assignment IDs are generation identifiers (`main.rs:3561`).

**[S] Coverage:** probes 1–4 version/create/assign/get help; 5–7 scratch joins; 8–15 creation, effort/deadline, held assignment, inspection, accept, legacy GO and start; 16–24 expected missing-reason and stale-token failures plus replacement/reopen; 25–28 fixture verdicts/message lint; 29–34 optional override omission and verbose get; 35–37 GO inbox-count observation; 38–39 block/update help; 40–41 parent/anchor/scaffold/sunset persistence. Fixture verdicts are not design approvals. Expected exit 1 occurs only at probes 16 and 19. The first wrapper attempt failed an extra assertion that an old assign message remained in the *current* owner inbox; this was an invalid historical-ID test after supersession. It was removed; the fresh successful rerun compares assign/get IDs directly and classifies the output from source. Failure retained in `round1-probe-initial-failure.log:1`; no failed run is counted as passing.

**[S] Probe 1, actor `lead`; `mesh version --json` — exit 0.**

```text
{
  "version": "0.2.29",
  "git_commit": "6789201c5511b51be704fe30c6e4d025f3e64f8c",
  "git_dirty": false,
  "build_time_utc": "2026-09-07T13:25:52Z",
  "protocol_version": 1,
  "schema_version": 1
}
```

**[S] Probe 8, actor `lead`; `mesh task create --subject 'Measure assignment rendering' --description 'Objective: preserve all five lines. Review route: measure; acceptance lead.' --first-step 'Read scratch candidate.txt' --deliverable 'scratch result.md with field coverage' --completion-signal 'RESULT with findings or BLOCKED with reason' --effort high --why 'reference identity requires careful checking' --deadline 30 --lane-id rendering --work-kind verification --criticality supporting --json` — exit 0.**

```text
{
  "id": "1",
  "subject": "Measure assignment rendering",
  "description": "Objective: preserve all five lines. Review route: measure; acceptance lead.",
  "status": "pending",
  "metadata": {
    "completion_signal": "RESULT with findings or BLOCKED with reason",
    "criticality": "supporting",
    "deadline_minutes": "30",
    "deliverable": "scratch result.md with field coverage",
    "effort": "high",
    "effort_why": "reference identity requires careful checking",
    "first_step": "Read scratch candidate.txt",
    "lane_id": "rendering",
    "work_kind": "verification"
  }
}
```

**[S] Probe 9, actor `lead`; `mesh task assign 1 --owner seat --awaiting-go` — exit 0.** **The `(id: …)` below is a message ID, not the assignment token; see the following JSON get.**

```text
[mesh] assigned task #1 -> seat (pending) (id: e37e308c-44f4-4bb5-b7b7-41b720f1ae05)
```

**[S] Probe 10, actor `lead`; `mesh task get 1 --json` — exit 0.**

```text
{
  "id": "1",
  "subject": "Measure assignment rendering",
  "description": "Objective: preserve all five lines. Review route: measure; acceptance lead.",
  "status": "pending",
  "owner": "seat",
  "metadata": {
    "assigned_at": "2026-09-08T02:36:02.617Z",
    "assigned_by": "lead",
    "assignment_id": "f73b95b5-c8c8-4e8b-8a50-6504c2806277",
    "awaiting_go": "f73b95b5-c8c8-4e8b-8a50-6504c2806277",
    "completion_signal": "RESULT with findings or BLOCKED with reason",
    "criticality": "supporting",
    "deadline_minutes": "30",
    "deliverable": "scratch result.md with field coverage",
    "effort": "high",
    "effort_why": "reference identity requires careful checking",
    "first_step": "Read scratch candidate.txt",
    "lane_id": "rendering",
    "work_kind": "verification"
  },
  "lifecycleStage": "assigned",
  "countsAsActiveLane": false,
  "pendingEffort": false,
  "deadlineMinutes": "30"
}
```

**[S] Probe 11, actor `lead`; `mesh task get 1` — exit 0.**

```text
#1 [pending] Measure assignment rendering
Lifecycle: assigned
Owner: seat
Assignment: f73b95b5-c8c8-4e8b-8a50-6504c2806277
Counts as active lane: no
Description: Objective: preserve all five lines. Review route: measure; acceptance lead.
First step: Read scratch candidate.txt
Deliverable: scratch result.md with field coverage
Completion: RESULT with findings or BLOCKED with reason
Effort: high — reference identity requires careful checking
Deadline: 30 minutes
Metadata keys: assigned_at, assigned_by, awaiting_go (+3 more)
Use --json or --verbose for the full task record.
```

**[S] Probe 13, actor `lead`; `mesh task update 1 --go` — exit 0.**

```text
updated task #1 -> pending
```

**[S] Probe 16, actor `lead`; `mesh task assign 1 --owner seat2` — exit 1.**

```text
stderr:
error: invalid name 'admin_reason': reassigning an owned or already-assigned task requires --admin-reason
```

**[S] Probe 17, actor `lead`; `mesh task assign 1 --owner seat2 --admin-reason 'move isolated review to second seat' --first-step 'Read scratch replacement.txt' --deliverable 'scratch replacement-result.md' --completion-signal 'RESULT replacement or BLOCKED' --effort medium --why 'bounded replacement' --deadline 20 --awaiting-go` — exit 0.** **The `(id: …)` below is a message ID, not the assignment token; see the following JSON get.**

```text
[mesh] assigned task #1 -> seat2 (pending) (id: 27c0afa9-0bf4-4546-aabf-04ff96de545f)
```

**[S] Probe 18, actor `lead`; `mesh task get 1 --json` — exit 0.**

```text
{
  "id": "1",
  "subject": "Measure assignment rendering",
  "description": "Objective: preserve all five lines. Review route: measure; acceptance lead.",
  "status": "pending",
  "owner": "seat2",
  "metadata": {
    "assigned_at": "2026-09-08T02:36:02.777Z",
    "assigned_by": "lead",
    "assignment_id": "91766efb-7bbd-4e18-b8f7-ba1287909409",
    "awaiting_go": "91766efb-7bbd-4e18-b8f7-ba1287909409",
    "completion_signal": "RESULT replacement or BLOCKED",
    "criticality": "supporting",
    "deadline_minutes": "20",
    "deliverable": "scratch replacement-result.md",
    "effort": "medium",
    "effort_why": "bounded replacement",
    "first_step": "Read scratch replacement.txt",
    "lane_id": "rendering",
    "work_kind": "verification"
  },
  "lifecycleStage": "assigned",
  "countsAsActiveLane": false,
  "pendingEffort": false,
  "deadlineMinutes": "20"
}
```

**[S] Probe 19, actor `seat2`; `mesh task accept 1 --assignment f73b95b5-c8c8-4e8b-8a50-6504c2806277` — exit 1.**

```text
stderr:
error: assignment token mismatch: provided 'f73b95b5-c8c8-4e8b-8a50-6504c2806277', current assignment is '91766efb-7bbd-4e18-b8f7-ba1287909409'; current owner: seat2
Current contract:
  First step: Read scratch replacement.txt
  Deliverable: scratch replacement-result.md
  Completion signal: RESULT replacement or BLOCKED
```

**[S] Probe 22, actor `seat2`; `mesh task complete 1 --summary 'isolated fixture complete'` — exit 0.**

```text
[mesh] completed task #1
[#1 Measure assignment rendering · owner: seat2]
Assignment: 91766efb-7bbd-4e18-b8f7-ba1287909409
Summary: isolated fixture complete
```

**[S] Probe 23, actor `lead`; `mesh task assign 1 --owner seat --reopen --admin-reason 'bounded resumed stage' --awaiting-go` — exit 0.** **The `(id: …)` below is a message ID, not the assignment token; see the following JSON get.**

```text
[mesh] assigned task #1 -> seat (pending) (id: ef79a82b-cc34-44fe-ac18-9b92af535ec3)
```

**[S] Probe 24, actor `lead`; `mesh task get 1 --json` — exit 0.**

```text
{
  "id": "1",
  "subject": "Measure assignment rendering",
  "description": "Objective: preserve all five lines. Review route: measure; acceptance lead.",
  "status": "pending",
  "owner": "seat",
  "metadata": {
    "assigned_at": "2026-09-08T02:36:02.913Z",
    "assigned_by": "lead",
    "assignment_id": "b74f2ac9-6cb5-4d5b-b40b-3648a9545054",
    "awaiting_go": "b74f2ac9-6cb5-4d5b-b40b-3648a9545054",
    "completion_signal": "RESULT replacement or BLOCKED",
    "criticality": "supporting",
    "deadline_minutes": "20",
    "deliverable": "scratch replacement-result.md",
    "effort": "medium",
    "effort_why": "bounded replacement",
    "first_step": "Read scratch replacement.txt",
    "lane_id": "rendering",
    "reopen_reason": "bounded resumed stage",
    "reopened_at": "2026-09-08T02:36:02.913Z",
    "reopened_by": "lead",
    "work_kind": "verification"
  },
  "lifecycleStage": "assigned",
  "countsAsActiveLane": false,
  "pendingEffort": false,
  "deadlineMinutes": "20"
}
```

**[S] Probe 25, actor `seat`; `mesh task ruling 1 --kind verdict --value accepted --ref fixture-candidate` — exit 0.**

```text
[mesh] recorded ruling #1 for task #1
```

**[S] Probe 26, actor `seat`; `mesh task ruling 1 --kind verdict --value rejected --ref fixture-candidate` — exit 0.**

```text
[mesh] recorded ruling #2 for task #1
```

**[S] Probe 27, actor `lead`; `mesh send seat 'ACTION REQUIRED: Task #1 release; first step: inspect fixture; completion signal: RESULT'` — exit 0.**

```text
[mesh] lead -> seat: sent (id: e7621bd8-6ef9-4108-a9f7-8995730b727d)
stderr:
[mesh] warning: actionable message missing required fields: deliverable
```

**[S] Probe 29, actor `lead`; `mesh task create --subject 'Optional override omission' --first-step 'Read fixture' --deliverable result.md --completion-signal RESULT --json` — exit 0.**

```text
{
  "id": "2",
  "subject": "Optional override omission",
  "status": "pending",
  "metadata": {
    "completion_signal": "RESULT",
    "deliverable": "result.md",
    "first_step": "Read fixture"
  }
}
```

**[S] Probe 32, actor `lead`; `mesh task create --subject 'Effort without reason' --first-step 'Read fixture' --deliverable result.md --completion-signal RESULT --effort low --json` — exit 0.**

```text
{
  "id": "3",
  "subject": "Effort without reason",
  "status": "pending",
  "metadata": {
    "completion_signal": "RESULT",
    "deliverable": "result.md",
    "effort": "low",
    "first_step": "Read fixture"
  }
}
```

**[S] Probe 33, actor `lead`; `mesh task assign 3 --owner seat2 --why 'optional supplied reason'` — exit 0.**

```text
[mesh] assigned task #3 -> seat2 (pending) (id: dd734c1e-30b4-4f83-b4d8-fdf49adcf220)
```

**[S] Probe 37, actor `lead`; `mesh task update 4 --go` — exit 0.**

```text
updated task #4 -> pending
```

**[S] Probe 40, actor `lead`; `mesh task create --subject 'Scaffold metadata preservation' --description 'Inspect synthetic scaffold metadata' --first-step 'Read fixture' --deliverable fixture-note.md --completion-signal 'RESULT or BLOCKED' --work-kind scaffolding --lane-id rendering --criticality scaffolding_only --parent 1 --anchor 2 --scaffold-class fixture_note --sunset-decision archive --sunset-owner lead --sunset-trigger 'fixture accepted' --json` — exit 0.**

```text
{
  "id": "5",
  "subject": "Scaffold metadata preservation",
  "description": "Inspect synthetic scaffold metadata",
  "status": "pending",
  "metadata": {
    "anchor_task_ids": [
      "2"
    ],
    "completion_signal": "RESULT or BLOCKED",
    "criticality": "scaffolding_only",
    "deliverable": "fixture-note.md",
    "first_step": "Read fixture",
    "lane_id": "rendering",
    "parent_task_ids": [
      "1"
    ],
    "scaffold_class": "fixture_note",
    "sunset_decision": "archive",
    "sunset_owner": "lead",
    "sunset_trigger": "fixture accepted",
    "work_kind": "scaffolding"
  }
}
```

**[S] Probe 41, actor `lead`; `mesh task get 5 --json` — exit 0.**

```text
{
  "id": "5",
  "subject": "Scaffold metadata preservation",
  "description": "Inspect synthetic scaffold metadata",
  "status": "pending",
  "metadata": {
    "anchor_task_ids": [
      "2"
    ],
    "completion_signal": "RESULT or BLOCKED",
    "criticality": "scaffolding_only",
    "deliverable": "fixture-note.md",
    "first_step": "Read fixture",
    "lane_id": "rendering",
    "parent_task_ids": [
      "1"
    ],
    "scaffold_class": "fixture_note",
    "sunset_decision": "archive",
    "sunset_owner": "lead",
    "sunset_trigger": "fixture accepted",
    "work_kind": "scaffolding"
  },
  "lifecycleStage": "unassigned",
  "countsAsActiveLane": false,
  "pendingEffort": false
}
```

**[S] Exact token comparison output** (`round1-token-check.log:1`):

```text
assign probe 9: message_id=e37e308c-44f4-4bb5-b7b7-41b720f1ae05; get probe 10: assignment_id=f73b95b5-c8c8-4e8b-8a50-6504c2806277; distinct=True; message-id classification: main.rs:3561-3565
assign probe 17: message_id=27c0afa9-0bf4-4546-aabf-04ff96de545f; get probe 18: assignment_id=91766efb-7bbd-4e18-b8f7-ba1287909409; distinct=True; message-id classification: main.rs:3561-3565
assign probe 23: message_id=ef79a82b-cc34-44fe-ac18-9b92af535ec3; get probe 24: assignment_id=b74f2ac9-6cb5-4d5b-b40b-3648a9545054; distinct=True; message-id classification: main.rs:3561-3565
```

**[S] GO delivery observation** (`probe-observations.txt:1`):

```text
GO inbox counts before={'seat2.json': 4, 'lead.json': 4, 'seat.json': 3}; after={'seat2.json': 4, 'lead.json': 4, 'seat.json': 3}; equal=True
```

**[I — UNVERIFIED design] Decision age:** the optional wait context names the decision owner, what decision/artifact is pending, and the time that request became pending, separately from notice delivery/read time. If that request time is absent, display `age unknown`; do not substitute assignment time or infer an idle interval from silence. Preserve owner and age on task-get/Scope and directed release where useful. This is the adjudicated gap at `docs/design/mesh-next-adjudication.md:69`, and requires a reader test with delayed delivery to settle correctness.

## Recommendation

### Measurement plan and estimator

**[P] Original ceiling, unchanged:** the threads study reports 62 task bodies, zero exact duplicates within them, and an assignment consolidation ceiling of 7,485 characters / 4 = **1,871.25 TE**, or **8.9%** of its 84,524-character task sample (`docs/design/mesh-task-threads-research.md:75`). The character counts were independently reproduced here [S; `rebuild-index.log:23`]; the claimed opportunity remains a counterfactual ceiling, not evidence of consumed tokens saved. It assumes a combined message can preserve everything without exceeding the retained prose size. That assumption is not granted to a proposed renderer for free.

**[S] Two reproducible offline illustrations:** `python3 .check-logs/mesh-next-phase0/render-pairs.py` exited 0; `rendered-metrics.json:1` contains the sums of body strings only. The synthetic fixture and historical GO are separate obligations and excluded from both nine-pair populations. The first corpus retains authored source text verbatim (with line-5 tails relocated), including doctrine for an audit of lossless preservation; it is not the production instruction to paste doctrine. The second replaces enumerated generic checkout/reviewer doctrine with a pinned link, as standard:27 requires. All nine second-illustration bodies are in `linked-doctrine-cards.md:1`, from `nine-linked-cards.json:1`; each substitution and its authority is explicit in `linked-doctrine-map.json:1`. The required custom score schema, commands, waits, budget semantics, lane constraints and independent-review restrictions remain in the bodies. `round1-standard.md:1` retains the linked bytes; SHA-256 is carried in each link. No fetching or executing source commands is part of rendering.

| Quantity [S calculation] | Old authored + card | Verbatim audit illustration | Linked-doctrine illustration |
|---|---:|---:|---:|
| Unicode characters | 23,101 | 25,627 | 27,167 |
| UTF-8 bytes | 23,147 | 25,713 | 27,271 |
| TE = characters / 4 | 5,775.25 | 6,406.75 | 6,791.75 |
| Net removed characters (negative = added) | — | -2,526 | -4,066 |
| Net removed UTF-8 bytes | — | -2,566 | -4,124 |
| Net removed TE | — | -631.5 | -1,016.5 |
| Net / original 84,524-character task sample | — | -2.99% | -4.81% |

**[S calculation / I — UNVERIFIED design judgment]** Neither illustration attains the historical 8.9% ceiling. The explicit conflict routes and missing-state annotations cost text; the second adds repeated immutable-link identities, exceeding its removed doctrine. These are measured payload increases, not model-cost observations. This tests the ceiling's assumption instead of treating it as guaranteed savings. The old 22,140-character / 1.14% illustration is superseded because it omitted operative commands and invented holds; it is not a valid preservation baseline. Do not weaken commands, wait authority, conditional budgets or review isolation to recover a percentage (`rendered-metrics.json:1`; `d2-opus-findings.md:5`; standard:27, :53; machinery-review-astra.md:210).

**[I — UNVERIFIED design] Unit of accounting:** one authored/card pair **per assignment generation per exposure path**, keyed by archive/team incarnation or resolved source identity, task ID, full assignment token, obligation kind, recipient/context generation and path. Paths are directed inbox, optional task-get output, and human Scope; a model tool reading Scope is a separate model exposure. Fresh assignment, same-owner reissue, owner change and cold recovery remain distinct. A pre-GO contract and its GO release have different obligations even under the same token. Do not match solely on task title, task ID, latest snapshot or similar body text. Basis: machinery review `:210`, archive supersession `archive-task-inventory.txt:34`, adjudication `:15`.

**[I — UNVERIFIED design] Offline comparison method:** freeze the same source cut and per-generation field inventory for control and candidate; render both without any agent calls. For each eligible generation/path, measure `C_before = chars(authored) + chars(card)` and `C_after = chars(combined)` only if both old exposures are actually evidenced on that path. Otherwise report generated/stored sizes separately, with exposure coverage unknown. Count GO, correction, resume, follow-up retrieval, envelope/header and repair-query text in their own strata on both sides. Define net payload delta as `Σ C_before − Σ C_after − added_headers − added_retrievals − added_repairs`, avoiding double-counting overhead already inside a body. The aggregate record should retain both gross removed and introduced characters. Basis: prior study's marginal accounting discipline (`docs/design/mesh-messaging-overhaul-research.md:198`) and paired-generation warning (`machinery-review-astra.md:210`).

**[P/I — UNVERIFIED estimator model] TE disclosure:** use Unicode code-point count divided by four to remain comparable with the original study; **not bytes/4** and not billed tokens (`docs/design/mesh-task-threads-research.md:26`). Report UTF-8 bytes separately. Sensitivity at /3 and /5 yields net TE -842.00 / -505.20 for the verbatim illustration and -1355.33 / -813.20 for linked doctrine; each ratio remains unchanged when numerator/denominator use the same estimator. If later permitted, report actual family tokenizer counts separately from TE, without replacing the baseline estimator retroactively. Model billing requires actual input/output/cache-write/cache-read records; there is no correlation-based output saving multiplier.

**[I — UNVERIFIED design] Future behavioral comparison:** after operator authorization, compare an equally disciplined two-body control with this single-card treatment, stratified by work kind, model/family, generation type and wait status; use independent equivalent fixtures to avoid learning from repeated prompts. Predeclare observation windows from assignment delivery through accepted completion or the same blocking cutoff. Count model turns, clarification turns, pure acknowledgments, incorrect starts while waiting, stale-generation attempts, result correctness, review-route omissions and reference mismatches separately from body sizes. Use paired per-fixture differences where valid and report sample size and uncertainty; no numeric percentage target is invented by this document. Acceptance requires no loss of contract fields or wait/isolation correctness; fewer model turns is a hypothesis to test, not an assumed consequence of fewer bodies. Basis: standard `:19`, `:53`, machinery review `:210`, overhaul `:198`.

**[U — UNVERIFIED] Wave-2 economics:** no final wave-2 inbox/workflow/turn source cut was opened in this lane. The machinery review explicitly could not measure double exposure for its 68 task IDs (`docs/design/field-test-wave2/machinery-review-astra.md:210`). A permitted immutable wave-2 export must include task creation/assignment/supersession/release records, retained body IDs and physical origins, sender/recipient and path, delivery/read observations, context/turn IDs, actual token classes, and coverage gaps. Only then can we count observed pairs per generation/path, missing bodies, repeated retrievals, GO notices and actual turns. Commits, task counts, generated bodies and `read: true` are not substitute exposure counts. Safe default: keep all wave-2 savings and model-turn effects unquantified.

### Open questions for the architect charter

| Question [U — UNVERIFIED] | What would settle it | Safe default / proposed decision [I — UNVERIFIED design] |
|---|---|---|
| Does `description` suffice as the objective carrier for all existing producers? | Inventory producer-owned descriptions and test lossless display on long/multiline descriptions; Mesh currently truncates them at 160 characters in human get (`main.rs:1561`). | Reuse description; do not add a mandatory second objective. Until human get becomes lossless, use JSON/verbose inspection and preserve full legacy text; never infer an objective from its truncated preview. Author normalization is needed only at new combined-card assignment. |
| How will creation accept review route and optional context ergonomically? | Bounded CLI design review and implemented help/isolated probes; current help lacks those typed inputs (probe 2). | Add input support for the named fields, not more required fields; never advertise a speculative flag as shipped. No authored second message as the normal workaround. |
| Is a scalar review-route sufficient for later joins/ACLs? | Paired consumer design and restricted-review conformance; adjudication `:51` already requires visibility enforcement. | Keep fifth-line text complete; optional structured projection may supplement it later, with no guessed reviewer membership or expanded audience. |
| Will paired owners approve the proposed TaskCommand::Go surface and the legacy migration? | Repair/paired review of the existing handler, whose GO input does not carry a generation token (`main.rs:3265`, `:3316`), and an isolated stale-release test. | Use the new lifecycle surface decision above, pending approval; require expected-generation compare-and-commit and explicit delivery. Today's unguarded update path is not an implementation substitute. Leave recorded waits unchanged on failed validation. |
| Can Taurhaus honor the UUID-valued wait on every path? | Isolated cross-repo reader tests and the phase-0 contract-repair gate; source mismatch documented above. | Gate rollout on agreement. Keep canonical token string and fail closed for conflicting wait representations; no local silent boolean conversion. |
| Which bytes are the immutable rubric/candidate/packet for pending reviews? | Lead-issued full immutable revisions/digests and allowed-evidence manifest at release, with source object availability verification (`standard:90`). | Pre-read may proceed on stated material; no review execution/acceptance until frozen. Abbreviated historical hashes remain abbreviated evidence, not invented full SHAs. |
| What resolves main/master and added-lines/total-lines in the sample? | Contemporaneous authoritative correction or acceptance-owner ruling on that exact generation; pairs 1/2/5/7 and 3 expose the conflicts. | Preserve both values; visible conflict plus task block / named BLOCKED route requests the authoritative ruling for this generation. Execution reports recorded state, never an invented hold or a renderer-chosen counting bound. Never “repair” the archive. |
| What distinguishes cold resume from a new stage? | Owner/generation/lifecycle/cursor contract tests, including completed-stage reopen (probes 22–24; standard:45). | Same unchanged generation keeps its token and wait; new stage contract/reopen receives a new generation. Ambiguous next action requires clarification, not replay of a stale start. |
| Must a given task carry a budget script or retained packet copy? | Task's actual leash/evidence declaration and owner-approved counting/retention policy (adjudication:57, :61). | Do not demand them universally; for declared budgets preserve method and tool identity, and mark missing historical values unavailable. Distinguish archived evidence from a mere reference; no implicit source deletion. |
| Will consolidation reduce turns or total cost on each path? | Matched source/exposure/turn archive or an authorized controlled trial with the accounting above. | Report the offline characters/bytes illustration only; preserve the original wave-1 ceiling's scope and make no wave-2 saving claim. |
| Is additive release packaging sufficient? | Mesh/Taurhaus paired contract review of validation, aliases, JSON shape, GO notice semantics and legacy callers (`overhaul:217`). | Optional data additions may be additive; changed behavioral meanings wait for explicit review. Leave the locked versions and standing teams alone. |

### Explicit spec-deltas for acceptance

**[I — UNVERIFIED design]** These are proposed behavior changes, not shipped enforcement or approvals. Owning requirements remain binding; the following owners must accept changes before implementation/rollout (brief:118–120; overhaul:217).

| Delta | Owning document / current evidence | Acceptance owner |
|---|---|---|
| Require five nonempty lines on combined-card assignment, including description objective and new `metadata.review_route`; old records remain inspectable. | standard:19–25; current three-field resolver `main.rs:310`. | Orchestrator for delivery contract; paired Mesh/Taurhaus contract owners for validation behavior. |
| Exempt identified generated assignment/GO messages from the full-contract `deliverable:` substring lint; retain legacy lint until migration. | standard:31; `main.rs:689`; brief:89. | Orchestrator and Mesh message-contract owner, with paired consumer review. |
| Introduce TaskCommand::Go compare-and-commit plus generated assignee release body, distinct from pre-GO; decide legacy update migration. | standard:53; current `main.rs:3273`, `:3316`; overhaul:217. | Paired Mesh/Taurhaus contract owners; orchestrator accepts operational release semantics. |
| Expose assignment ID separately from inbox message ID in assign output and proposed JSON, without changing old `id` meaning. | `main.rs:3561`; fresh probes 9/10, 17/18, 23/24. | Mesh CLI owner and paired consumers/orchestrator. |
| Add optional structured-context writers, generation resets/snapshots and exact `assignment_context` clear entry; default unknown-key access policy. | `task_lifecycle.rs:8`, `:214`, `:267`; standard:53, :66, :90; adjudication:51. | Mesh lifecycle owner and paired Taurhaus consumers; orchestrator approves scope. |
| Route stopping/isolation to manifest Context/References; fifth line stays work kind/acceptance/reviewers/seam. Replace bundled lead duplicate-authoring rules with generated-card flow. | standard:25–27, :90; lead role `src-tauri/resources/templates/roles/v3-lead-claude.yaml:205`. | Orchestrator/delivery-standard owner. No widening of line 5 is requested. |
| Make human get lossless for the contract and share authorized rendering with inspection surfaces. | `main.rs:1561`; standard:19; adjudication:15, :51. | Paired CLI/UI contract owners; rollout after isolated long-text and restricted-review tests. |

### Bounded implementation acceptance, after review

**[I — UNVERIFIED design; basis: brief `docs/design/assignment-rendering-brief.md:11`, standard:19, adjudication:66]** The next lane should implement the record inputs and one renderer, connect existing assignment/get surfaces, add the generation-bound GO rendering only after paired contract approval, and replace the small set of lead-role assignment rules. It should not implement transport, push, subscriptions, a shared thread, a summarizer, build-host coordination or a new required contract field. No implementation or corpus edit was performed here.

**[I — UNVERIFIED design] Acceptance fixtures should prove:** (1) all five lines and every declared optional field survive creation→assignment→get without truncation; (2) missing effort/reason/deadline stays optional; specifically `first_step`/`firstStep` and `completion_signal`/`completionSignal` resolve with defined precedence, while `deliverable` has **no camelCase alias** (`main.rs:320`, `:331`, `:345`); (3) both T9 generations retain their own tokens and pre-read waits, and GO changes only the matching generation; (4) two different owner contexts and cold resumes cannot act on stale tokens; (5) immutable candidate/rubric/packet refs never resolve to a later mutable tip; (6) independent reviewer projections disclose no peer verdict or derivative; (7) assignment committed/delivery failed remains visibly undelivered; (8) actual byte/character/token estimators and exposure-path counts use the declared populations. These are future checks, not a gate this researcher ran. Their source rationale is the matrix, archive supersession (`archive-task-inventory.txt:34`), current delivery failure path (`main.rs:3539`), and adjudication `:51`.

**[I — UNVERIFIED design] Review disposition:** submit this design to the orchestrator for Fable altitude review and the decorrelated Opus defect lens, one round, as commissioned in `docs/design/assignment-rendering-brief.md:35`. The supplied round-1 Opus defect review was reached and returned fix_required (`d2-opus-findings.md:1`); this document applies that one fix round. Orchestrator verification, altitude acceptance and operator GO have not been observed by this researcher. Confidence is high in the reconstructed pair counts, observed binary contract and need to preserve waits; moderate in this bounded schema/rendering choice; unknown in net production token/turn savings. The recommendation is to approve a contract-preserving implementation lane only after the named compatibility decisions, not to authorize rollout from this research result.

### Round-1 artifact verification

**[S] Executed verification:** `python3 .check-logs/mesh-next-phase0/verify-report.py` exited 0. The final output below is quoted from `verify-report.log:1`; assertions compare the pinned source archive and source-authored spans to rendered bodies, not just the report to its own JSON. The executed-command assertion is restricted to the Executed command evidence section, so archive/proposal quotations are not misrepresented as commands run. An earlier check caught an unmatched backtick introduced by truncating an ACTION REQUIRED heading; the renderer now uses a concrete bounded heading without slicing command text. Its initial failure is retained in `verify-report-initial-failure.log:1`. These are artifact checks, not a product gate.

```text
PASS: pinned archive matches all 18 source bodies; exact assertion 15616/7485; 45 authored fields and 9 contexts covered against source, with line-5 relocation checked.
PASS: all 5 archived mesh command occurrences restored verbatim; 9 generation tokens, recorded-state labels and listed generated-card additions checked.
PASS: 9 historical cards, 9 linked-doctrine cards and 1 synthetic card conform to the literal heading order; optional References/Budget/Change/Effort/Deadline exercised.
PASS: 22 command quotes checked only inside Executed command evidence; quoted outputs equal captured outputs; 41 empty-environment scratch-root invocations, expected exit-1 probes 16 and 19 only.
PASS: assign message IDs differ from task-get generation tokens in all 3 cases; parent/anchor/scaffold/sunset keys persist in fresh probe 41.
PASS: linked-doctrine substitutions preserve remaining source spans and commands; retained standard digest and both body-only estimators checked; historical GO source and template checked.
METRIC verbatim: 25627 chars, 25713 bytes, 6406.75 TE; net removed -2526 chars (-2.99% of 84524).
METRIC linked: 27167 chars, 27271 bytes, 6791.75 TE; net removed -4066 chars (-4.81% of 84524).
Scope: source-to-artifact checks only; semantic completeness beyond checked spans, product implementation, model uptake and orchestrator approval are not established by this script.
```

### Round-1 fix record — 2026-09-08

**[S]** Findings 1–13 have the design-level fixes indexed below; OQ1–OQ3 are addressed and OQ4 stays explicitly open. Locations name this revised document and the reproducible sibling evidence. **[U — UNVERIFIED]** Orchestrator verification/acceptance of the fixes is not claimed; that independent review settles closure beyond these observed artifact checks. No binding contract, wait, counting or isolation rule was relaxed to make a check pass.

| Finding | Exact fix location | Fix applied [S; proposed behaviors remain I above] |
|---|---|---|
| 1 | `assignment-rendering-design.md:239, assignment-rendering-design.md:259, assignment-rendering-design.md:297, assignment-rendering-design.md:356`; `verify-report.py:67` | Five archived commands restored verbatim; command-execution assertion limited to executed evidence; real verification output above. |
| 2 | `assignment-rendering-design.md:78` | Matching awaiting_go gates WAIT; context is descriptive only. Conflicts name the BLOCKED lifecycle route; historical cards report recorded status, no invented hold. |
| 3 | `assignment-rendering-design.md:88, assignment-rendering-design.md:195, assignment-rendering-design.md:378` | One literal template and shared renderer for all nine cards plus optional-field synthetic card; Reconciliation and Record metadata are named slots. |
| 4 | `assignment-rendering-design.md:410, assignment-rendering-design.md:461`; `round1-token-check.log:1` | Assign stdout message IDs distinguished from task-get generation IDs, including replacement/reopen; assign token output added to compatibility envelope. |
| 5 | `assignment-rendering-design.md:42, assignment-rendering-design.md:54` | Added parent/anchor/scaffold/sunset and archived ruling/verdict/artifact/review fields, plus lossless authorized unknown-key policy and restrictive review default. |
| 6 | `assignment-rendering-design.md:58` | Defined each subkey lifetime; add exact top-level assignment_context clear entry and reconstruct only explicit task-scoped whitelist/current generation values. |
| 7 | `assignment-rendering-design.md:58, assignment-rendering-design.md:5` | Named create/assign/block/ruling/proposed Go writers, first-assignment drafts and assign-time wait payload semantics. |
| 8 | `assignment-rendering-design.md:812` | Enumerated behavioral deltas, owning requirements and acceptance owners; no approval claimed. |
| 9 | `assignment-rendering-design.md:33, assignment-rendering-design.md:195` | Kept fifth-line scope at standard:25; task-specific stopping/isolation and checkout tails retained verbatim in Context/manifest. |
| 10 | `assignment-rendering-design.md:136` | Selected proposed new TaskCommand::Go lifecycle surface; legacy update path explicitly unguarded and pending migration review; corrected conditional-warning premise with source. |
| 11 | `assignment-rendering-design.md:201`; `render-pairs.py:35` | Concrete action headings for executable work; pre-read-only headings for both T9 generations. |
| 12 | `assignment-rendering-design.md:834`; `verify-report.py:20` | Source-to-render field/context/command coverage assertions; log names checked scope and semantic limits instead of circular preserved claim. |
| 13 | `assignment-rendering-design.md:830` | Explicit first_step/firstStep and completion_signal/completionSignal fixtures; deliverable has no camelCase alias. |
| OQ1 | `assignment-rendering-design.md:772` | Second linked-doctrine nine-card illustration, exact substitution ledger and pinned standard, same bytes/chars/TE estimator. Neither illustration reaches the historical ceiling. |
| OQ2 | `assignment-rendering-design.md:140` | Real release projected separately, retaining candidate 8f97e28, rubric 6a44056, 57→60 checks, R1→R10, 47(d) deferral and candidate-referenced ruling. |
| OQ3 | `assignment-rendering-design.md:405` | Named mesh_task.rs:233 boolean fixture as contract-repair lane work; no repository test edited here. |
| OQ4 | `assignment-rendering-design.md:800` | Retained objective/truncated-get open question with JSON/verbose safe default and isolated long-text producer/read tests to settle it. |
