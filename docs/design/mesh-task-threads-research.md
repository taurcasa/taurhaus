**Task-scoped communication in Mesh — commissioned Astra research**

2026-09-07. Research only; prepared for orchestrator review. Measurements below come from local archived records and source inspection. Counterfactual costs and design judgments are labeled separately. Only this memo was written; no Mesh binary, live team state, daemon, or tmux server was used.

**Recommendation: adapt.** Give tasks a bounded, source-linked read projection, alongside directed inbox delivery. Start with the assignment, current state, rulings, results, waits, and changes since a reader's cursor. Make history available on demand. Do not introduce a second conversation database or subscribe every agent to every task. Shared visibility could eliminate relays and stale instructions, but the measured archive does not support an automatic token saving from shared reading: even delta reads cost more unless readership stays selective or the information being delivered becomes substantially smaller.

The distinction that matters is between **being able to inspect shared evidence** and **putting that evidence into every participant's model context**. The first is promising; the second must earn its cost. A thread is also insufficient as an uptake contract: a seat needs the current assignment and release condition, not the responsibility of adjudicating a chronological conversation.

**Evidence and its limits.** These are the sources actually inspected:

| Source | Use and boundary |
|---|---|
| [Commissioning brief](mesh-task-threads-brief.md) | Required questions and the claimed family input/output behavior. |
| [Wave-1 archive directory](/home/mstie/projects/taurjob/docs/wave-1/mesh-archive/) and its `taurjob-team-archive.tar.gz` | All retained inbox arrays; workflow, protocol, and mutation journals inside the tarball; adjacent task snapshots and telemetry inventory. The archive was streamed in memory, without extraction. |
| [Token accounting](field-test-wave1/token-accounting.md) | Recovered full-wave usage, summed across pre/post-outage sessions. Rounded seat totals, not per-message billing or an experimental response curve. |
| [Retro package](field-test-wave1/retro-package.md), [Astra audit](field-test-wave1/machinery-audit-astra.md), [Fable audit](field-test-wave1/machinery-audit-fable.md) | Failure cases, corrections to earlier diagnoses, and full-wave overhead. Their different cutoffs and task-touch definitions are retained rather than silently averaged. |
| [Team retro](/home/mstie/projects/taurjob/docs/wave-1/retro.md), [judge contribution](/home/mstie/projects/taurjob/docs/wave-1/retro/judge-astra-1.md), [heavy contribution](/home/mstie/projects/taurjob/docs/wave-1/retro/heavy-implementer.md), [ledger](/home/mstie/projects/taurjob/docs/wave-1/ledger.md) | Concrete stale-message examples and the successful restart/release practices. Later-wave accounts are not added to the early archive census. |
| [Mesh USAGE](/home/mstie/projects/mesh/USAGE.md) and source files linked below | What is already implemented in the checked-out source. This is not a claim about the binary or daemon currently running. |
| [Delivery standard](../team-delivery-standard.md) | Current assignment, correction, response-expectation, and restart conventions. |
| [Scope synthesis v2](team-comprehension-view/synthesis.md) and [design proposal](team-comprehension-view/design-proposal.md) | Existing consumer requirements; no UI design is proposed here. |

Repository heads at inspection: taurhaus `b123303892601f70d0fb570bee2770f2181e68d4`; taurjob `e8abcf6ef7e7bab97cd66627ec6a5e82d9cab5ff`; mesh `6789201c5511b51be704fe30c6e4d025f3e64f8c`. The inspected Mesh files had no local modifications. Archive SHA-256: `60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1`. Token-accounting SHA-256: `5e4cdfc3ba40af04ed58d9c062dcb745be5eb0c06ea5734edf39747c8586de3e`. The reproduction code below identifies the exact sample and coding decisions.

The archive is a complete retained inbox snapshot of the nine-seat team, **not a complete transcript of the nine-seat wave**. Its envelopes span 2026-09-06 19:34:31.772–20:11:26.180 UTC, about 36m54s. Its adjacent task snapshot contains T1–T11; later ledger work reaches T28. The full-wave token report covers both team generations. Neither the full-wave message total nor the fraction of the full-wave bill removable by threads can be recovered from these inbox arrays.

**The measured traffic contains 106 envelopes and 200,190 body characters.** An envelope means one array entry, including generated and native-harness messages, whether read or unread. Body size is the Unicode character count of `text`, including nested JSON text; outer envelope metadata is excluded. Token figures for this archive use characters/4, labeled **token equivalents (TE)**. They are not tokenizer measurements. Characters/3 or /5 would move absolute TE estimates by +33% or −20%; ratios using the same estimator are unchanged. Different rendering and family tokenizers remain additional uncertainty.

| Retained inbox | Envelopes | Body characters | Unread (`read: false`) |
|---|---:|---:|---:|
| `architect.json` | 20 | 39,130 | 0 |
| `heavy-implementer.json` | 15 | 25,766 | 0 |
| `heavy-implementer-1.json` | 14 | 33,267 | 3 |
| `implementer-1.json` | 13 | 25,067 | 1 |
| `judge-astra-1.json` | 8 | 10,988 | 0 |
| `team-lead.json` | 34 | 65,466 | 34 |
| `lead-taurjob.json` | 2 | 506 | 2 |
| `design-taurjob.json` | 0 | 0 | 0 |
| `product-reviewer.json` | 0 | 0 | 0 |
| `altitude-reviewer.json` | 0 | 0 | 0 |
| **Total** | **106** | **200,190** | **40** |

The ten filenames represent nine seats plus the lead alias. All 34 `team-lead` bodies decode as native `idle_notification` JSON, containing seat reports or lead roundups; they are not 34 ordinary direct-message bodies. The two `lead-taurjob` envelopes are monitor escalations. An unread flag does not prove that the information never reached a model through another harness path. Equally, the three empty inboxes do not establish that those seats received no messages.

That limitation is independently visible in the archived workflow journal: **86 distinct `message_sent` events**, of which only **28** have an exact message-ID match in the retained inbox arrays. The unmatched 58 comprise 36 addressed to `lead-taurjob`, nine to design, seven to altitude, and six to product-reviewer. This is a missing-body join, not proof of 58 lost deliveries: consumption, external rewrites, and native transport differences cannot be distinguished here. There are also 37 `message_read` and 36 `message_delivery_recorded` events; neither is a count of model turns or empty polling attempts.

I manually classified retained envelopes by their principal communicative purpose. Task-scoped includes an explicit task seam or acceptance request; one primary task is assigned for costing, with secondary references left as links. A task mentioned inside a whole-team status roundup does not make that roundup a task post. This avoids copying one long lead report into five different hypothetical threads.

| Classification | Envelopes | Share of envelopes | Body characters | Share of body | TE |
|---|---:|---:|---:|---:|---:|
| Task-scoped action, evidence, decision, or wait | 62 | 58.5% | 84,524 | 42.2% | 21,131.00 |
| Team broadcast, retained recipient copies | 10 | 9.4% | 4,075 | 2.0% | 1,018.75 |
| Seat onboarding | 15 | 14.2% | 79,700 | 39.8% | 19,925.00 |
| DM-like operational exchange or cross-task roundup | 19 | 17.9% | 31,891 | 15.9% | 7,972.75 |
| **Total** | **106** | **100%** | **200,190** | **100%** | **50,047.50** |

“DM-like” describes scope, not confidentiality: these operational reports should not automatically become public task commentary. The two broadcasts concern commit hygiene and the directory change. Each has five retained copies; the archive does not justify treating five as the original broadcast audience. Of the 62 task-scoped envelopes, 45 are in GPT-seat inboxes, two are monitor escalations to the lead, and 15 are task-focused native reports in `team-lead`. Including those 15 costs the retained encoded body exactly as stored; decoding and compact rendering could reduce it.

Only **12/106 envelopes (11.3%)** have structured `taskId`: nine assignment cards, one nudge, and two escalations. Thus **50/62 semantically task-scoped envelopes lack that field**. The workflow journal has 26 task-linked send records, but only 12 join retained bodies by exact ID. Task names in prose are useful research evidence; they are not a safe replacement for explicit task association in a production projection.

**Duplication is real, but its location matters.** Exact body equality produces 88 distinct bodies, six repeated-body groups, and 18 surplus envelope copies. Removing all but one body from each exact group would remove 56,768 characters, **28.4% of retained text**. That is a storage/content-reuse ceiling, not a promise of saved recipient reads.

| Exact repetition | Retained copies | Surplus characters | Interpretation |
|---|---:|---:|---|
| Heavy onboarding | 3 copies of 5,231 characters | 10,462 | Repeated to the same seat. |
| Second-heavy onboarding | 5 copies of 5,253 | 21,012 | Repeated to the same seat, including two later notices. |
| Architect onboarding | 3 copies of 5,216 | 10,432 | Repeated to the same seat. |
| Sol onboarding | 3 copies of 5,801 | 11,602 | Repeated to the same seat. |
| Commit-hygiene broadcast | 5 copies of 382 | 1,528 | Same body across five recipients. |
| Directory broadcast | 5 copies of 433 | 1,732 | Same body across five recipients. |

Same-seat onboarding repetition accounts for **53,508 characters**, 94.3% of exact surplus text. Some replay may be necessary after a fresh context or relaunch; the snapshot cannot establish that all ten extra onboarding copies were wasted. Nevertheless, versioned onboarding and one combined assignment delivery are better-supported places to investigate avoidable repetition than indiscriminate task subscriptions. The exact cross-recipient surplus is only **3,260 characters**, 1.6% of all retained text, and those broadcast recipients still needed the new rule. A shared stored body reduces copies on disk; five agents reading it still costs five reads.

Cross-recipient overlap also exists below whole-message equality. There are **22 distinct identical body lines of at least 80 characters** appearing in multiple inboxes: 3,121 unique characters and 7,697 characters of additional cross-seat incidence. This measures boilerplate reuse, including onboarding and broadcasts. It excludes shorter/paraphrased overlap and ignores repeated occurrences within the same seat; it overlaps the preceding counts and must not be added to them as another saving.

Within the **62 task-scoped bodies, there are zero exact duplicate bodies**. There is nevertheless demonstrable duplicated purpose: nine assignments appear both as an authored contract and a generated assignment card. The nine prose contracts total 15,616 characters and the cards 7,485, together 5,775.25 TE. If one combined delivery preserved all contract fields and the assignment token, eliminating the second rendered card would avoid up to **1,871.25 TE, 8.9% of task text**. This is a consolidation opportunity for the current assignment mechanism; it does not require a shared thread. The two versions are not semantically interchangeable in every field, so blind deletion is inappropriate.

Other observed overlap is semantic and is not assigned an invented duplicate-token percentage: the architect receives the combined T3/T4 acceptance request at 19:44:54, another T4 request at 19:45:46, and another reminder to rule on T4 alongside T5 at 19:46:41. T8 acceptance is sent by the architect at 19:55:12, while the lead's 19:55:32 next-assignment message still describes the acceptance as owed. These are plausible savings from checking shared current state before sending, not evidence that every repeated statement is dispensable.

**The communication failures have different causes.** The following distinguishes what raw threads change from what a task projection plus the new controls could change. Archive references use inbox name and UTC timestamp; later-wave examples are attributed to the retro/audits.

| Failure and evidence | Would shared conversation have prevented it? | Mechanism that actually addresses it; remaining risk |
|---|---|---|
| Onboarding tells the seat to acknowledge; assignment cards prohibit acknowledgments before work. Seen in archived heavy onboarding and T9's card; Astra audit §9. | **No.** Putting both in one place exposes the contradiction and may make it more salient. | Correct the instruction source; show one operative contract and an explicit dependency wait. A thread does not decide precedence. |
| T9 card at 19:51:20 and prose at 19:51:31 require pre-reading and waiting for GO. Monitor says resume at 19:57:01.555; GO arrives 19:58:00.249. | **No for an ordinary thread.** The premature instruction would join the history. | The new assignment-bound `awaiting_go` marker suppresses the operational nudge. Projection shows wait and authorized release separately. The interval is **58.694 seconds**. |
| T7 altitude pre-read/wait reports at 19:40:39 and 19:42:58 are in `team-lead`, yet inactivity is diagnosed and the lead substitutes a review. | **Possibly reduces misdiagnosis**, if the lead actually sees the wait evidence. It does not establish launch health or prove work uptake. | Canonical routing plus recorded wait state. Astra's primary-evidence correction overrides the Fable audit's “never acknowledged” account. Do not transfer T9's measured 59-second interval to T7; the retro package conflates those nearby examples. |
| Lead inbox split: 34 native notifications versus two monitor escalations. | **Only if the projection deliberately covers both sources.** A canonical-only query can preserve the blind spot. | Current Mesh resolves the alias and warns about a historical alias file; it does **not** merge its history or control foreign writers. An archive importer must retain both physical origins. Fix external routing separately. |
| Dependency-parked T27/T28 lanes repeatedly nudged; T23 held for T20-run; later-wave retro and Fable F12. | **No.** More thread checks could turn a legitimate wait into more responses and apparent activity. | Explicit blocked state/reason and GO metadata suppress idle nudges. Release should name the dependency artifact and assignment. Runtime-health escalation remains separate from operational resumption. |
| T12 reassignment leaves two reviewers producing competing artifacts; former-owner accept/start succeeds, completion refuses; E10, Astra §§1,3. | **May reveal the conflict earlier**, but a thread cannot prevent a stale owner from acting. | Current owner/assignment validation and terminal-state controls are load-bearing. Old-generation posts stay historical and cannot authorize work. Per-seat worktrees address the file collision. |
| Reviewer work invisible in routing rows: T7/T9 verdicts recorded on T1/T2; Fable F9 and Astra §4. | **Improves discoverability, not accounting by itself.** | Join ruling author, candidate reference, subject task, and review task/assignment. Existing rulings give author and reference; they do not necessarily give the review work unit, active time, or tokens. A thread length is not reviewer effort. |
| Superseded rubric and acceptance references: T9 pre-read says 57 checks, GO supplies a 60-check rubric; T6 report corrects an already-complete task and stale hash at 19:51:44; architect is told R5 is already settled at 19:52:12. | **Helps only with current-state selection.** A chronological thread contains more obsolete answers. | Pin candidate/rubric hashes in the assignment; show explicit supersession. Never infer that the newest acceptance nullifies all older findings. |
| Post-commit loose-file warnings overlap; judge sends fresh status after warnings at 05:35:02 and 05:36:09, following its 05:34:46 commit. Judge retro. | **Could avoid some parallel requests and repeated replies.** The last warning was 83 seconds after the commit; actual blocked time is unknown. | One release fact with path/hash/fresh status, checked by would-be senders. Delayed delivery still requires revalidation; the thread is not a checkout lock. |
| Full-wave instrument recursion and long ledger cells: R41 stop, 257 ledger revisions, 21k–24k-character cells; both audits. | **Could worsen it.** Shared commentary makes it easier to add another finding/review and harder to see the stop condition. | Frozen review contract, bounded rounds, and the existing depth decision. Generate current views from facts instead of adding thread summaries to the same maintenance burden. |
| Shared checkout contamination, wrong display capture, broken gates, resource contention. | **No.** Visibility can warn but cannot isolate files, displays, or build slots. | Worktree/lease/validation controls. Do not credit a conversation feature with preventing these mechanical failures. |
| Independent T2 reviews: T9 explicitly must not see product-reviewer's findings before committing its own verdict. | **An unrestricted T2 thread would make this worse.** | Separate review-task audiences until the required verdicts are committed; expose candidate/rubric facts without exposing the other review. Shared visibility must respect the review route. |

These cases support both operator intuitions. Shared evidence can eliminate needless relays and expose already-resolved requests. Unfiltered history also carries superseded instructions, tempting findings from other reviewers, and machine nudges whose authority must not exceed the task contract. The archive supplies examples of each; it supplies no controlled estimate of net response reduction.

**Mesh already has most of the useful structured history.** Source inspection supports a projection-first design, with specific limits:

| Existing source | What it can contribute | What it cannot establish |
|---|---|---|
| Task JSON, metadata, owner, dependencies, assignment ID; [task commands](/home/mstie/projects/mesh/src/main.rs:3351) | Current contract, lifecycle, ownership, budget/ruling/artifact fields, GO marker. | Current metadata alone is not every previous value or its effective interval. Acceptance owner/reviewer routes can remain prose. |
| Assignment card and protocol contract | First step, deliverable, completion signal, task identity, assignment token. | A string mentioning several tasks is not a typed seam or subscriber policy. |
| [Workflow journal](/home/mstie/projects/mesh/src/workflow.rs:39) | Event IDs; assignment generations; accepts/starts/progress/blocks/reviews/completions; attributed rulings; send/read/delivery facts. Archive: **321 events**, including 12 assignments, 23 rulings, and seven completions. | `MessageSent` stores routing/intent/task fields, **not the local message body**. Lifecycle summaries are retained, but arbitrary conversation is not recoverable from these events. External-message observations have richer envelopes; that does not fill local-message gaps. |
| [Protocol index](/home/mstie/projects/mesh/src/protocol_index.rs:18) | Query by task/message ID; actionable fields; append-only upsert rows with latest version by record ID. Archive: **53 rows**, all lifecycle intents. | No general body store, no complete set of native messages, and its latest-record query is not a historical conversation replay. |
| [Task mutation journal](/home/mstie/projects/mesh/src/task_journal.rs:13) | Task, actor, timestamp, changed-field names; append order and byte-offset readers. Archive: **98 rows**. | **No before/after values.** It cannot reconstruct an old description, cleared wait marker, or arbitrary overwritten metadata. It is an invalidation/correlation source, not full event sourcing. |
| [Rulings and artifacts](../team-delivery-standard.md) / [workflow ruling schema](/home/mstie/projects/mesh/src/workflow.rs:196) | Attributed, sequenced evidence that survives reassignment; candidate references; explicit budget rulings where recorded. | A ruling survives because it judges work, not because it belongs to the current assignment. Review-task linkage and explicit resolution of a previous finding are not guaranteed. |
| RESULT/BLOCKED compatibility recognition in [task get](/home/mstie/projects/mesh/src/main.rs:2493) | Latest matching completion signal from the resolved lead inbox, without marking it read. | The parser expects an exact `RESULT 9`/`RESULT #9` header and a following payload, or a numeric BLOCKED header with reason. Archived `RESULT T9 <hash>` prose and JSON-encoded native reports do not automatically match. It is not generation-validated acceptance. |
| [Task completion fan-out](/home/mstie/projects/mesh/src/main.rs:4065) | Completion state plus one packet, routed to lead, assigner, and owners of explicitly blocked successor tasks; recipient set deduplicated, author omitted. | Delivery occurs after completion state is committed and can fail. This already provides some shared-result routing; a new thread must not duplicate it or imply all recipients consumed the result. |
| [Idle-monitor records](/home/mstie/projects/mesh/src/idle_monitor.rs:600), [USAGE declared waits](/home/mstie/projects/mesh/USAGE.md:684) | `source=idle_monitor`, kind, seat, task, timestamp, message ID; launch-health references ignored nudge IDs. Survives inbox supersession. | Monitor observations are not worker decisions. Old archives predate this schema, and deadline/manual events are separate sources. |
| [Existing projections](/home/mstie/projects/mesh/src/projections.rs:80) | Lead board, assignee views, recovery bundles, attention, taurhaus workflow projection. | These are principally current-state views. Some getters ensure/rebuild persisted projections: “query” must not be assumed to mean a mutation-free operation during research. |
| [Task cursor](/home/mstie/projects/mesh/src/task_cursor.rs:8) | Owner-written atomic sidecar: last command/result/next action, writer and timestamp; recovery adds its resume hint. | It is a **restart hint, not a per-reader consumption cursor**. It is absent from journals and removed best-effort on completion. Its schema does not structurally hold all the standard's cwd/hash/root fields or an assignment ID. |
| [Inbox storage](/home/mstie/projects/mesh/src/inbox.rs:106) and [message schema](/home/mstie/projects/mesh/src/types.rs:13) | Retained authored bodies, message IDs where present, delivery read flags. Generated unread notices supersede by task/kind; authored messages append. | Native/foreign rewrites can defeat retention. Read flags are delivery consumption, not a task-history watermark. An unread notice replacement is not an auditable correction relation. |

For ordinary authored sends, [the CLI](/home/mstie/projects/mesh/src/cli.rs:51) has no task/audience/reply-to option; [the implementation](/home/mstie/projects/mesh/src/main.rs:563) can extract a task link from an explicit `[orchestration_v1]` block into the workflow record. Merely writing “T9” does not provide that link. `cmd_send` leaves the inbox's task fields empty. Its action-contract lint defaults to warning, and each broadcast recipient gets a different message ID. Therefore a production projector should not deduplicate different authored events by text equality: the same words may be a legitimate repeat, and broadcast copies currently lack one common post ID.

The useful first projection is complete only for its declared inputs: current task facts plus retained structured events and safely linked bodies. **A complete conversation projection is not available for free.** Missing pieces, if a trial establishes a need, are:

1. Explicit authored-message task association, including multiple-task seams and the relevant assignment generation; one stable logical-post ID across fan-out; reply/correction links where required. Start by using existing explicit protocol task references, then add a small structured writer convenience only if that is insufficient.
2. Durable retention of opted-in shared bodies or immutable artifact references. The workflow journal and protocol index do not contain them. This could be an extension of an existing canonical event/record, with inboxes as delivery projections; it need not be a parallel thread database. Do not silently archive every DM to obtain completeness.
3. Audience and independent-review release semantics. Historical participant inference is useful for research but cannot authorize production disclosure. There is no demonstrated per-task access boundary in the current shared-filesystem model.
4. A per-reader, durable multi-source watermark, with source identity, version/offset, and a recovery rule for truncation, replacement, malformed rows, late arrivals, and assignment changes. Reusing the restart-cursor habit helps agents remember it; overloading the owner's restart sidecar would make readers overwrite one another.
5. Typed links among review work, candidate, finding/ruling, and resolution where telemetry needs that claim. A newer timestamp cannot supply a missing relationship or manufacture acceptance.

A rebuildable derived index/cache is compatible with projection-first. It is not a new authority. Preserve source IDs and missingness, and keep writes governed by the task lifecycle. Do not materialize a second editable status or a second version of the assignment inside the conversation.

**Read economics: the following is a replay of retained task text, not a reconstructed invoice.** The baseline is disciplined targeted inbox consumption: every retained task body enters its addressed seat's context once. This is deliberately a reasonably operated inbox baseline, not an assumption that today's agents must reread their entire inbox. Fixed onboarding, broadcasts, and cross-task reports remain unchanged in every comparison: **28,916.50 TE**. Work artifacts and task-card reads common to both designs cancel; additional cards, summaries, notices, and extra model turns must be added back.

Task text totals **U = 21,131 TE**, 62 bodies, mean 340.82 TE/body. For each primary task, sort bodies by timestamp. If its N bodies have sizes u₁…uₙ, a single reader checking the entire growing history after every arrival consumes `Σ(N−i+1)uᵢ`. A delta reader consumes `Σuᵢ`. This model keeps all historical generations of T9, treats each body as one post, and assumes no paraphrase/relay elimination unless explicitly varied. It also counts the native report bodies as retained, rather than inventing their original messages.

| Primary task | Bodies | Once-through TE | One-reader full-history TE, check after each arrival |
|---|---:|---:|---:|
| T1 | 12 | 5,075.25 | 32,863.50 |
| T2 | 4 | 1,881.25 | 5,303.75 |
| T3 | 5 | 931.50 | 3,388.75 |
| T4 | 5 | 947.50 | 3,244.75 |
| T5 | 5 | 997.00 | 3,448.75 |
| T6 | 5 | 3,606.25 | 11,186.75 |
| T7 | 6 | 2,537.50 | 8,786.25 |
| T8 | 4 | 959.50 | 2,561.75 |
| T9 | 8 | 2,152.50 | 10,418.75 |
| T10 | 4 | 1,038.00 | 2,909.75 |
| T11 | 4 | 1,004.75 | 3,249.00 |
| **Total** | **62** | **21,131.00** | **87,361.75** |

Full reading amplifies a single reader's task text **4.134×**, before widening the audience. For a frequency sensitivity, checking every task at its first event, each 60 seconds until its last event, and once at the last event yields **155 checks and 194,099.25 TE per reader**. At five-minute intervals it yields **44 checks and 57,061.50 TE**. These are simulated schedules, not observed polling frequencies. Both stop at the last event; every subsequent idle check adds that task's entire final history again. Delta reads return no old body on an empty check, although command/result overhead is still payable.

For an archive-grounded participants case, use the union of senders and recipients in each task's coded messages, canonicalize `team-lead` to the real lead, exclude the monitor as a reader, and include the lead. This yields 30 reader-task pairs: 16 Claude-family and 14 GPT-family. It is a retrospective audience estimate, not a proposed ACL: it misses unretained participants, includes T9's former owner, and gives members visibility before they necessarily joined. Independent-review restrictions are not modeled as an excuse to deliver less than the declared content. A second case skips a reader's own authored posts, which a practical agent reader should do by default.

| Delivery/read policy | Claude task TE | GPT task TE | Total task TE | Task ratio to inbox | Total including fixed other traffic |
|---|---:|---:|---:|---:|---:|
| Targeted inbox, once per retained envelope | 8,520.25 | 12,610.75 | **21,131.00** | **1.00×** | **50,047.50** |
| Participants, deltas, all posts | 36,112.50 | 20,132.00 | 56,244.50 | 2.66× | 85,161.00 |
| Participants, deltas, skip own posts | 15,982.25 | 19,456.50 | **35,438.75** | **1.68×** | **64,355.25** |
| Participants, full history each arrival | 150,805.75 | 88,057.50 | 238,863.25 | 11.30× | 267,779.75 |
| Participants, full history, skip own posts | 65,227.00 | 86,997.25 | 152,224.25 | 7.20× | 181,140.75 |
| All nine seats, deltas, all posts | 84,524.00 | 105,655.00 | 190,179.00 | 9.00× | 219,095.50 |
| All nine seats, full history each arrival | 349,447.00 | 436,808.75 | 786,255.75 | 37.21× | 815,172.25 |

These costs describe what happens **if** shared visibility becomes shared consumption. Making the same evidence available on demand need not incur these reads. Conversely, keeping full inbox bodies and also reading their thread representation is additive; the table assumes replacement of those reads, not a free overlay.

**Family coupling changes the bill, but a correlation is not a conversion factor.** The full-wave report records lead output of **5.16M tokens** and **1,292M cache reads**, versus **1.774M output** across the five GPT seats. That establishes the lead as the largest generation source in these totals. The report attributes its large cache-read burden to accumulated notices. It does not attach token spans to notice IDs, so it cannot establish that all 1.292B cache reads were avoidable notice rereads. Specifications, tools, reasoning history, and notices share the retained context. No percentage of 1.292B is claimed as a thread saving here.

The brief supplies approximately **0.95 input/output correlation** and the premise that Claude output is input-coupled. The named token-accounting file contains no correlation calculation or per-turn observations. Recomputing Pearson correlation on its rounded seat totals gives **r = 0.915** for Claude cache reads versus output (four seats), **r = 0.963** for GPT total input versus output (five seats), and **r = −0.116** for Claude fresh-plus-cache-write input versus output. The cache-inclusive slopes are about 0.00387 and 0.00232 output tokens per input-class token, respectively, but compare different roles and workload durations. These aggregate associations neither validate a causal family response law nor refute an unavailable per-turn study. In particular, **r = 0.95 does not mean 0.95 extra output tokens for each input token**. Claude sensitivity is taken seriously below as an explicit scenario, rather than priced as a measured causal coefficient.

Use this general cost expression, with family/model-specific rates supplied by the operator's actual billing arrangement:

```text
Cost = Σ_family [(new_input × new_input_rate)
              + (cache_write × write_rate)
              + (cache_read × read_rate)
              + (output × output_rate)]

For marginal coordination text in a controlled comparison:
M_family = new_exposure_rate + h × cache_read_rate + β_family × output_rate
ΔCost ≈ Σ_family (Δintroduced_text_family × M_family)
        + extra summaries + extra checks/wakes/turns − avoided coordination work
```

Here `h` is the assumed subsequent cached appearances of introduced text before compaction, and `β` is additional generated output per additional introduced coordination token under otherwise equal work. These are distinct from the number of times a tool fetches the same thread. Cached old context still costs; another tool response containing old text can introduce another copy into the context. The harness's actual caching and compaction behavior must be measured to avoid counting the same exposure twice. A cold resume also has a different cache profile.

To make the tradeoff numerically costed without inventing vendor prices, the next table uses **normalized cost units**: one unit purchases 1,000 fresh input tokens, output costs five times fresh input, both families have the same base input price, and `h=0`. Set GPT's assumed β to 0.05 and vary Claude's. These are sensitivity assumptions, not current prices or estimates derived from r. The published approximately 2.5× Astra/Sol premium is not a Claude/GPT price ratio and does not identify cache/output prices; therefore it cannot produce a defensible dollar invoice for this experiment. The TE table permits repricing by seat/model once those rates are known.

| Assumed Claude β; GPT β=0.05 | Targeted task cost | Participant delta cost, skip own | Participant full-history cost, skip own | Delta cost if replacement bodies retain 50% of text |
|---|---:|---:|---:|---:|
| 0 | 24.284 | 40.303 (1.66×) | 173.974 (7.16×) | 20.151 (0.83×) |
| 0.25 | 34.934 | 60.281 (1.73×) | 255.507 (7.31×) | 30.140 (0.86×) |
| 1.00 | 66.885 | 120.214 (1.80×) | 500.109 (7.48×) | 60.107 (0.90×) |

Claude's marginal input multiplier in these assumptions is 1, 2.25, or 6; GPT's is 1.25. With βClaude=0.25, twenty subsequent cache appearances at a hypothetical 0.1 input-price cache rate would make those multipliers 4.25 and 3.25 instead. Actual cache-write rates would be added where relevant. The absolute bill depends on these assumptions; the fact that uncompressed participant deltas increase **both** families' input is robust to positive pricing differences. Greater Claude coupling makes extra Claude readership especially expensive; it also increases the value of removing lead relays when that removal is demonstrated.

The 50%-retained column is an optimistic counterfactual: the same content obligations are met with half the rendered text, without extra summary authorship or card/check cost. It buys only 10–17% in this normalized example. Adding a new 200-TE current card for all 30 reader-task pairs costs **6,000 TE**, exceeding the raw 3,411.625-TE saving from halving the author-excluded participant deltas. If the card replaces a card already read, that cost cancels; if it is another artifact the agent must read, it does not. An LLM writing and repeatedly revising these summaries would consume still more input/output. This is why the smallest trial should compile existing facts, not employ a permanent thread summarizer.

**Break-even can be stated precisely, including length and check frequency.** Let a task have N equal-size logical posts of u TE. Let `d` be the targeted-delivery burden per logical post, including actual fan-out and truly redundant relays; let `P` be consuming readers per post. Let `s` be the fraction of the targeted source material retained after safe consolidation. For C equally spaced full-history checks while posts arrive evenly, plus H checks after the thread is complete:

```text
Targeted payload D = dNu
Thread delta payload = PsNu
Thread full payload  = PsNu × [(C+1)/2 + H]

Delta wins when: d > Ps, before extra overhead.
Full wins when:  d > Ps[(C+1)/2 + H].
With one check per post (C=N) and H=0: N < 2d/(Ps) − 1.
With a check every k posts: C=N/k, so N < k[2d/(Ps) − 1].
```

For variable sizes, use the measured weighted-prefix sum above rather than `(N+1)/2`. With new response/header cost K, the available payload saving times its family marginal price must also exceed K. An empty delta check is not free if it wakes a model into generating a response; event-driven reads and a no-response convention are part of the economics.

On the retained task bodies there is **no measured exact-repeat benefit**: the literal replay has d=1, s=1. Two or three consuming readers already lose at the first full read, and never reach a delta break-even. Reducing the nine duplicate assignment deliveries alone saves at most 8.9% of task payload, well below the reduction required by the participant replay. Larger semantic relay savings remain plausible but unmeasured.

For a concrete favorable hypothetical, suppose five targeted relay/delivery equivalents carry each logical post (`d=5`), three readers need the shared post (`P=3`), and no shortening is assumed. Deltas save 40%; full reads beat targeted delivery only for **N≤2** at one check per post (N<2.33). With five posts between checks, **N≤10** wins at the completed check boundaries. One extra read after completion exhausts the favorable full-read case even for a one-post thread. If s=0.5 as well, the event-per-post threshold becomes **N≤5**. Threads therefore need delta discipline even when the operator's message-reduction intuition is strongly correct.

The archive permits more useful **family-specific** thresholds. Let s shrink each task's rendered bodies proportionally, leaving its participants and event schedule unchanged. Comparing each family's projected input against its own measured targeted input yields:

| Policy | Maximum retained fraction s for Claude break-even | Maximum s for GPT break-even |
|---|---:|---:|
| Participant deltas, read all posts | 23.6% (remove >76.4%) | 62.6% (remove >37.4%) |
| Participant deltas, skip own | **53.3% (remove >46.7%)** | **64.8% (remove >35.2%)** |
| Participant full reads, skip own | **13.1% (remove >86.9%)** | **14.5% (remove >85.5%)** |
| Participant full reads, all posts | 5.65% | 14.32% |

Equality is cost parity before overhead; a saving needs strict improvement. With flat input/output response within one family, its price multiplier cancels from this threshold: family identity alone does not change the number of checks that fit. Different readership and relay reduction do. Input coupling changes how strongly that family's gains/losses influence the combined bill. Under the normalized β scenarios above, author-excluded participant deltas need aggregate retained fractions below **60.3%, 58.0%, or 55.6%**, respectively. There is no honest universal “Claude wins after X messages; GPT after Y” without those audience, reduction, and response assumptions.

Finally, a bounded current-state summary can win with long threads even when raw history cannot. For one seat, if a recovery summary costs S TE to read, costs W to produce, and avoids J otherwise-required full reads of a history of L TE, its input-side value is `J(L−S)` times that seat's input cost, minus W and any omitted-fact repair. That comparison is against a seat that actually needs the history. It does not justify forcing summaries on seats that already have the answer in a correct assignment card.

**What an agent should receive at uptake.** Use the delivery standard's five-line contract: objective, deliverable, first action, completion signal, review route. Add the operative owner/assignment identity, canonical artifact/candidate and rubric references, counting semantics where relevant, current wait/release condition, and explicit unresolved rulings. Keep the first action visible. A changed instruction should update that operative contract and point to what it supersedes; chronology remains evidence behind it. No majority of replies, last speaker, or monitor message should override the named acceptance owner.

For ordinary task work, the owner, named acceptance owner, lead, and explicitly routed collaborators may inspect the shared task evidence. They need not all receive every progress note. The lead should see a compact cross-task queue of changed outcomes, blockers, decisions, and exceptions; entering a task should fetch its bounded delta. Sending the lead every thread body would reproduce the most expensive traffic pattern in the token report. Downstream owners need the accepted artifact and release condition for their dependency, not all discussion that led to it.

Whole-team access to non-private structured task state can remain useful, but whole-team **subscription** should not be the default. Genuine team-wide changes remain broadcasts or shared standing-rule records with directed notices; do not replicate a rule into every open task thread. Current `resolve_send_recipients` deliberately delivers broadcasts to inactive registered members as well, despite older “active members” wording in the CLI description. Resuming seats must still learn a changed standing rule. Broadcast body reuse does not remove that information obligation.

Private communication, cross-task operational exchanges, cross-team relays, and unreleased independent review findings retain their appropriate DM or restricted-review routes. Task-relevant decisions emerging there should be published once to the task by an authorized participant, as a decision and evidence link, without silently exposing the original DM. A task association alone must not mean consent to wider visibility. If “every participating agent can read everything” includes independent reviewers before verdict commitment, decline that part of the proposal: it contradicts the actual T9 contract.

The DM/thread boundary is **advisory for agent behavior under today's shared filesystem**, with enforceable validation possible at a future Mesh writer/reader boundary for task IDs, explicit audience, generation, and reply references. That is not a confidentiality guarantee against an agent reading files directly. A strict security boundary would require separate storage/access control and is beyond the smallest increment. The projection should fail closed for private or unreleased review bodies rather than infer visibility from a task mention.

**The new machinery complements this projection.** Alias resolution establishes the current lead route; it does not retroactively collect foreign alias writes. `awaiting_go` names the current assignment, and release clears the marker; a thread read or acknowledgment cannot release it. Explicit blocked status and dependency state suppress operational nudges. After two nudges without task uptake, the monitor records one launch-health event and stops repeating; reads/acknowledgments do not reset that counter. A new assignment or actual lifecycle uptake does. Source-tagged monitor records should appear once as observations linked to their message IDs, alongside separately identified deadline/manual events. The same event visible through an inbox, workflow event, and task metadata is one event with several sources, not three nudges.

The projection must not create a parallel activity model, mark tasks started because someone read history, infer release from an artifact appearing, or repair lifecycle state as a side effect. The latest Mesh completion command already combines an owned terminal transition with result fan-out; preserve its state-before-delivery distinction and show delivery failures honestly. These controls prevent classes of failures that communication layout alone cannot.

**Scope and telemetry can consume the same evidence without an agent-token bill.** A task history compiled by ordinary code is a good source for the planned walk/inspection surface and for joining rulings to their authors. Scope already requires bounded template-generated lines, source links, an explicit fold, first-observed read marks, and separate read freshness/source modification/event times. A task projection should carry those identities and freshness facts rather than generate another narrative. Source event time alone would hide late-arriving old records after a read mark; keep arrival/first-observed position too. Showing an event read must not clear a disputed ruling. A completed task does not prove its declared scope is fulfilled. Reviewer tokens/duration still need task/assignment attribution at collection time; no history renderer can recover them from a verdict count.

**Compaction requires two different records.** The owner maintains the restart hint at meaningful step boundaries and before long waits, following the existing standard: cwd, code/candidate/rubric/tool hashes, active root, pending rulings, next action, and any running probe identity needed to avoid relaunching it. Current `mesh task cursor` has only command/result/next/timestamp fields; those additional identities must be explicit in the existing handoff/artifact or supported by a later deliberate schema change. Do not pretend the current sidecar already stores them structurally.

Separately, each reader keeps its history watermark. On resume it reads the current operative card, verifies its identities, and requests changes after the watermark. The compact task digest should initially be **deterministically compiled from existing records**: last current assignment, current wait, latest progress/result, applicable rulings with unresolved conflicts, and source links. Recompute on those source changes, not on empty checks and not by appending another summary after each message. Mark stale/unavailable fields and the source coverage window. A newer verdict must not erase a prior rejection without an explicit resolution relation.

Where a human-like synthesis is necessary, the accountable owner writes or approves a bounded checkpoint at handoff/compaction or a real decision boundary, naming the covered source IDs/generation and the unresolved questions. Label it as authored interpretation, keep older evidence accessible, and budget its input/output. It cannot substitute for an absent artifact hash or a missing GO. No permanent summarizer seat is justified by this sample. A watermark advances only through items actually returned/consumed; pagination and output budgets cannot silently mark folded, unseen evidence read.

**The smallest testable increment is an offline task-evidence projection over the archived snapshot.** This is a proposed follow-on experiment, not an implementation performed by this memo. It needs no new Mesh write path, no daemon deployment, and no live-team mutation. A pure reader can join the current task snapshots, workflow events, protocol entries, existing rulings/monitor metadata, and retained explicitly linked bodies, then emit a bounded current card plus changes since a client cursor with source IDs and coverage gaps. It should label the 50 manually inferred task bodies as research associations; a production-mode output includes only explicit links or authorized mappings. It should report native alias content as a separate historical source rather than quietly merge identities.

The offline trial should compare three readings of the same evidence: current assignment/recovery plus directed inbox deltas as the control; current card plus task-projection deltas; and full thread history as a deliberately costly reference. Otherwise any benefit from simply fixing assignment cards will be falsely attributed to threads. Use T1's amendments, T9's generations/GO/isolation, T7's wait, and T6's stale acceptance/rubric reports. Report unavailable history rather than inventing later wave fixtures from prose.

Predeclare the following checks and evidence before a follow-on implementation:

| Question | Trial evidence and stop condition |
|---|---|
| Can the reader find the operative action? | For each selected historical checkpoint, identify owner, assignment, first action, current candidate/rubric, wait/release condition, and completion route; every answer cites source or says unavailable. Zero wrong owner/GO/candidate claims. Do not mistake end-of-snapshot task values for facts available at an earlier checkpoint. |
| Is history complete enough for the claim? | Count explicit links, inferred associations, unmatched send IDs, retained bodies, unresolved native envelopes, and historical metadata gaps. The 58 missing-body joins must remain visible. No “all task communication” label. |
| Are deltas honest across restart? | Synthetic records in isolated temporary fixtures test a late old-timestamp event, multi-page read, duplicate observation, same-task reassignment, journal replacement/truncation, malformed/truncated final row, and cleared wait. No silent skipped fact; no old generation treated as current authority; same cursor and unchanged sources return no old body. |
| Does consolidation reduce the relevant cost? | Tokenize the actual returned output for each participating harness where supported; otherwise retain labeled TE. Include cards, headers, summary generation, source fetches, extra wakeups and turns. Compare with the equally well-formed inbox control, not with a wall-of-history caricature. |
| Does sharing preserve independent review? | Pre-verdict T9 output includes candidate/rubric/GO but excludes product-reviewer's verdict/findings, including indirect lead roundups carrying them. Zero leakage through summary or linked expansion in the trial surface. |
| Does the projection invent state or billable work? | Original snapshot bytes unchanged; no read implies accept/start/GO; monitor versus deadline/manual sources remain distinct; shared event identities are counted once. Ruling authorship is visible, while unmeasured reviewer duration/tokens stay unknown. |

An offline pass can establish coverage, deterministic correctness, and payload costs. **It cannot establish that agents send fewer messages or that Claude β is positive.** If it passes, the next separately commissioned wave should run a bounded, matched trial: at least two task shapes per family with the same model/effort and assignment quality, one accepted result and one compaction/resume per arm where feasible. Record model version, task/assignment, active/waiting/review durations, introduced message tokens, cache input, output, duplicate requests/relays, unresolved contradictions, and acceptance quality. Count work done by the lead and reviewers as well as implementers. Do not use tasks of different sizes to declare a family cost winner.

For that trial, predeclare a decision threshold of **at least 20% lower total coordination cost per accepted comparable task**, including summary and recovery costs, with no wrong GO/ownership action, missed required update, leaked independent verdict, or degraded acceptance result. This 20% is a judgment chosen to exceed a marginal win that could disappear into overhead; it is not a measured effect size. Small matched samples provide directional evidence only. If the projection improves operator comprehension but misses agent-cost parity, keep it as an on-demand Scope/history consumer and retain targeted agent reads. Stop the agent-consumption experiment rather than adding a second summary/review loop to make its metric pass.

**Reproduction is possible without writing or extracting the archive.** Run this from taurhaus with Python's standard library. The primary-task map is the disclosed manual coding, not an automated claim about message meaning. Array indices are zero-based; `B`, `O`, and `D` are broadcast, onboarding, and DM/cross-task. It prints aggregates, not message bodies. It intentionally reads no archived control-auth files.

```python
import collections as co
import hashlib, json, math, statistics, tarfile
from datetime import datetime, timedelta
from pathlib import Path

archive = Path.home() / "projects/taurjob/docs/wave-1/mesh-archive/taurjob-team-archive.tar.gz"
assert hashlib.sha256(archive.read_bytes()).hexdigest() == (
    "60bd9ba90b941211821c14b36b10dc583ddf21e961ff80547ff223141d2643e1"
)
primary = {
    "heavy-implementer": {3:4, 4:4, 5:4, 7:8, 8:8, 9:4, 10:8, 11:11, 12:11, 14:11},
    "heavy-implementer-1": {3:5, 4:5, 5:5, 7:9, 8:9, 9:5, 10:9},
    "architect": {3:1, 4:1, 5:1, 6:1, 7:1, 9:4, 10:5, 11:1, 12:1, 13:1,
                  14:8, 15:1, 16:1, 18:1, 19:10},
    "implementer-1": {3:3, 4:3, 5:3, 6:3, 8:3, 9:10, 10:10, 12:10},
    "judge-astra-1": {2:9, 3:9, 4:9, 5:9, 7:9},
    "lead-taurjob": {0:11, 1:2},
    "team-lead": {5:7, 6:7, 8:6, 9:7, 12:6, 16:6, 17:6, 18:1,
                  20:6, 21:2, 22:7, 24:7, 25:7, 28:2, 31:2},
}
broadcasts = {"Commit hygiene: pathspec commits only",
              "Directory unified: taurjob is now a symlink to taurjobs"}
rows = []
with tarfile.open(archive) as tar:
    for member in tar.getmembers():
        if "/inboxes/" not in member.name or not member.name.endswith(".json"):
            continue
        seat = Path(member.name).stem
        inbox = json.load(tar.extractfile(member))
        print("inbox", seat, len(inbox), sum(len(r["text"]) for r in inbox),
              sum(not r["read"] for r in inbox))
        for index, original in enumerate(inbox):
            task = primary.get(seat, {}).get(index)
            category = ("T" if task else "O" if original.get("summary") == "operator_notice"
                        else "B" if original.get("summary") in broadcasts else "D")
            rows.append(dict(original, seat=seat, index=index, task=task,
                             category=category, te=len(original["text"])/4,
                             ts=datetime.fromisoformat(original["timestamp"].replace("Z", "+00:00"))))
    journals = {}
    for name in ("workflow_events", "protocol_index", "task_mutations"):
        body = tar.extractfile(f"taurjob-team/state/{name}.jsonl").read()
        journals[name] = [json.loads(line) for line in body.splitlines() if line.strip()]
        print("journal", name, len(journals[name]))
for category in "TBOD":
    group = [r for r in rows if r["category"] == category]
    print("category", category, len(group), sum(r["te"] for r in group))
assert len(rows) == 106 and sum(r["te"] for r in rows) == 50047.5
exact = co.Counter(r["text"] for r in rows)
print("exact", len(exact), sum(n-1 for n in exact.values()),
      sum(len(body)*(n-1) for body, n in exact.items()))
lines = co.defaultdict(set)
for r in rows:
    for line in set(r["text"].splitlines()):
        if len(line) >= 80:
            lines[line].add(r["seat"])
shared = {line: seats for line, seats in lines.items() if len(seats) > 1}
print("shared lines", len(shared), sum(map(len, shared)),
      sum(len(line)*(len(seats)-1) for line, seats in shared.items()))
sent = [r for r in journals["workflow_events"] if r["eventType"] == "message_sent"]
ids = {r.get("id") or r.get("msg_id") for r in rows}
print("sent/matched", len(sent), sum(r["message_id"] in ids for r in sent))
print("unmatched destinations", co.Counter(r["recipient"] for r in sent if r["message_id"] not in ids))
print("structured links", sum(bool(r.get("taskId")) for r in rows))

# Authored contract index, generated card index: duplicated assignment purpose.
pairs = {"architect": [(3,6)], "heavy-implementer": [(3,5),(8,7),(11,12)],
         "heavy-implementer-1": [(3,5),(8,7)],
         "implementer-1": [(3,6),(10,9)], "judge-astra-1": [(3,2)]}
prose_chars = card_chars = 0
lookup = {(r["seat"], r["index"]): r for r in rows}
for seat, entries in pairs.items():
    for prose_index, card_index in entries:
        prose_chars += len(lookup[seat, prose_index]["text"])
        card_chars += len(lookup[seat, card_index]["text"])
print("assignment prose/card characters", prose_chars, card_chars)
assert (prose_chars, card_chars) == (15616, 7485)

tasks = [r for r in rows if r["task"]]
assert len(tasks) == 62 and sum(r["te"] for r in tasks) == 21131
canonical = lambda seat: "lead-taurjob" if seat == "team-lead" else seat
claude = {"lead-taurjob", "altitude-reviewer", "product-reviewer", "design-taurjob"}
family = lambda seat: "C" if canonical(seat) in claude else "G"
baseline, delta, full, skip_delta, skip_full = [co.Counter() for _ in range(5)]
for r in tasks:
    baseline[family(r["seat"])] += r["te"]
one_full = 0
for task in sorted({r["task"] for r in tasks}):
    posts = sorted((r for r in tasks if r["task"] == task), key=lambda r: r["ts"])
    audience = {canonical(r[k]) for r in posts for k in ("from", "seat")
                if r[k] != "mesh-idle-monitor"} | {"lead-taurjob"}
    once = sum(r["te"] for r in posts)
    prefix = sum((len(posts)-i)*r["te"] for i, r in enumerate(posts))
    one_full += prefix
    print("task", task, len(posts), once, prefix, sorted(audience))
    for seat in audience:
        f = family(seat)
        delta[f] += once
        full[f] += prefix
        skip_delta[f] += sum(r["te"] for r in posts if canonical(r["from"]) != seat)
        skip_full[f] += sum((len(posts)-i)*r["te"] for i, r in enumerate(posts)
                            if canonical(r["from"]) != seat)
assert one_full == 87361.75
for label, values in (("baseline", baseline), ("delta", delta), ("full", full),
                      ("skip_delta", skip_delta), ("skip_full", skip_full)):
    print(label, dict(values), sum(values.values()))
    if label != "baseline":
        print("retention thresholds", {f: baseline[f]/values[f] for f in "CG"})
for seconds in (60, 300):
    count, total = 0, 0
    for task in {r["task"] for r in tasks}:
        posts = [r for r in tasks if r["task"] == task]
        time, end = min(r["ts"] for r in posts), max(r["ts"] for r in posts)
        checks = []
        while time < end:
            checks.append(time)
            time += timedelta(seconds=seconds)
        checks.append(end)
        count += len(checks)
        total += sum(sum(r["te"] for r in posts if r["ts"] <= time) for time in checks)
    print("period/checks/TE", seconds, count, total)
for beta_c in (0, .25, 1):
    price = {"C": 1+5*beta_c, "G": 1+5*.05}
    cost = lambda values: sum(values[f]*price[f] for f in "CG")/1000
    print("normalized cost", beta_c, cost(baseline), cost(skip_delta),
          cost(skip_full), .5*cost(skip_delta))
def pearson(x, y):
    dx = [v-statistics.mean(x) for v in x]
    dy = [v-statistics.mean(y) for v in y]
    return sum(a*b for a,b in zip(dx,dy))/math.sqrt(sum(a*a for a in dx)*sum(b*b for b in dy))
print("correlations",
      pearson([1292,861,477,392], [5.16,1.95,1.62,1.53]),
      pearson([283.3,78.7,97.3,58,17.9], [.733,.380,.373,.221,.067]),
      pearson([6.855,5.504,12.222,4.728], [5.16,1.95,1.62,1.53]))
```

**Confidence and open questions.** Confidence is high in the retained envelope/character counts, exact-duplicate counts, arithmetic under the disclosed replay assumptions, and the inspected source's storage limitations. Confidence is moderate in the primary-topic classification and which stale relays a projection would avoid. Confidence is low in any full-wave saving, prospective audience size, causal family β, or reviewer productivity comparison. This is one short, incomplete-retention sample from one team, embedded in a much longer wave.

The next decision needs answers to these questions:

- Where are the per-turn observations and definition behind the brief's approximately 0.95 correlation, and can family response be estimated while controlling for task type, role, duration, and cache exposure?
- How much of the body-retention gap comes from native consumption/rewrites versus absent transport integration? Which writers can provide an explicit shared-body reference without collecting private DMs?
- What membership and post-verdict release contract preserves independent review while letting implementers and the lead share decisions? How are former assignees and late joiners handled?
- Can existing contract/result/ruling improvements alone deliver the same reduction in duplicate requests? They must remain the experimental control.
- Which review-task and supersession links are already reliable enough for telemetry, and which need minimal typed metadata rather than inference?
- What are the actual per-model fresh/cache-write/cache-read/output rates and extra-turn costs for the operator's arrangement? Which changes reduce lead coordination output rather than merely moving text to another seat?

**Adapt task threads into a bounded read projection over Mesh's existing authority, retain directed inbox notifications and DMs, and defer new shared-message storage until explicit linking/retention needs are proven.** Commission the offline snapshot projection and three-way reading comparison first. It will produce a reproducible coverage report, current-action correctness results, and per-family payload/cost estimates; only a later controlled wave can establish fewer messages and a lower bill.


---

**Wave-2 evidence addendum (2026-09-08).** The commissioned wave-2
machinery review — `field-test-wave2/machinery-review-astra.md` —
grades every decision in this study against the first full wave run
under the round-5 machinery and records the proposed amendments
(§6–§7 there). Headlines binding on this document's next revision:
the stale current-view failures (#18/#20/#31) and the #65 mutable-RESULT contamination strengthen the bounded-projection and immutable-submission contracts; five documented wait/release pairs and the 68-rows-vs-64-touches gap join the fixture set. The original measurements and models above are unchanged and
keep their original populations.

---

**Phase-0 addendum (orchestrator, 2026-09-08).** Overhead item 2 (one
combined assignment rendering) was designed and measured in
`assignment-rendering-design.md`: a rendering that preserves every
contract field, the assignment token, effort/reason, references and the
operative wait is LARGER than the authored contract plus generated card
it replaces on the nine wave-1 pairs (−2.99% with verbatim doctrine,
−4.81% with linked-not-pasted doctrine, of the 84,524-character task
sample; same estimator). The 8.9% ceiling above remains a correct
ceiling for deleting the second body; it is not reachable while the
fields are preserved. The item is reclassified a correctness change
(one operative contract, token-bound wait, GO release carrying its
deltas, generation-scoped context). The exact-repeat onboarding surplus
(53,508 characters) stays an interval, 0–13,377 TE, until archived
context-generation records exist (`onboarding-card-design.md`).
