**Wave-1 machinery audit — TAURHAUS**

**Verdict:** the team delivered through substantive review and persistent correction, but the machinery depended too heavily on people repairing ownership, evidence, and state tracking. Keep the complementary review lenses. Reduce permanent staffing and repeated documentation, and repair deterministic controls before drawing model-cost conclusions.

Audit cutoff: taurjob commit `1107f30`, which records integration closure; shipping code `115e1e6`. Counts exclude the three seed commits and subsequent team retrospective contributions. This is a read-only machinery audit, not a fresh product certification.

Evidence shorthand: **L** = [ledger.md](/home/mstie/projects/taurjob/docs/wave-1/ledger.md); **R** = [rulings.md](/home/mstie/projects/taurjob/docs/wave-1/rulings.md); **C** = [closure report](/home/mstie/projects/taurjob/docs/wave-1/wave-closure-report.md); **B** = [binding blueprint decisions](/home/mstie/projects/taurhaus/docs/design/team-blueprints-frontier-era.md); **S** = [delivery standard](/home/mstie/projects/taurhaus/docs/team-delivery-standard.md); **A** = [mesh archive](/home/mstie/projects/taurjob/docs/wave-1/mesh-archive); **Q** = [routing report](/tmp/claude-1000/-home-mstie-projects-taurhaus/dcb9f91d-188a-46eb-a9d2-3f77b554b6ec/scratchpad/routing-report-wave1.txt). Archive-internal paths below refer to `taurjob-team/` inside its tarball.

**1. Highest consequence: ownership and landing gates were conventions that repeatedly failed.**

The directory move killed the team; restoration then left assignments pointing at a nonexistent repository. Subsequently, mesh accepted `accept` and `start` from a reassigned task’s former owner, refusing only completion. An already-closed task reopened through later lifecycle commands. These are orchestration failures, not evidence that the affected members lacked ability. [L: E7, E10, “Board reconciliation”; team-recreation.md.]

Shared-checkout rules also failed repeatedly. Sol landed incomplete dependencies; a heavy implementer landed a noncompiling red test scaffold; four product-reviewer documentation commits swept another lane’s work into master. One swept code change broke formatting. Git confirms production changes inside `f1e4866`, `39c12b9`, and `998fb5e`, despite their documentation subjects. [L: E13, E14, E16; those commits.]

The operative gate was often **acceptance after landing**, despite the blueprint’s pre-merge language. Direct commits to master were explicitly instructed; T28-5 was cherry-picked before its exact gate finished. Later successful checks repaired confidence, but cannot establish that master was continuously gated. [L: branch rules, T28-5, E13–E16; B: prediction 4.]

**Consequence:** otherwise independent lanes could invalidate each other’s builds, commits, and evidence. Worktree isolation and atomic ownership enforcement should precede more role instructions.

**2. Reviews caught real defects; independence and evidence quality were uneven.**

| Review route | Distinct finding and consequence | Evidence |
|---|---|---|
| Fable structural → Sol M2 | No dispatcher, production `MemoryStore`, buffer-until-exit parsing, and broken receipt replay. A green checkpoint could not deliver the advertised run behavior. A2 correction was required before UI work. | [T19-A findings 1–4](/home/mstie/projects/taurjob/docs/wave-1/reviews/T19-A-structural.md); L:E12 |
| Astra → Fable design | Forbidden filtering/sorting, unsupported research readers, missing historical model identity, same-run retry, and timer-derived progress. | [T2 GPT findings 2, 4–6, 10](/home/mstie/projects/taurjob/docs/wave-1/reviews/T2-gpt-review.md) |
| Astra → design acceptance evidence | Retry disclosed current settings’ CV while execution read the source hunt’s CV; whole-hunt verification counted only the first verifier. | [T20-run GPT findings M1–M2](/home/mstie/projects/taurjob/docs/wave-1/reviews/T20-run-gpt-review.md) |
| Fable structural → integration | Three separate task-row writers explained counter loss and inconsistent error representations; one shared writer became R31. | [T26 structural A1](/home/mstie/projects/taurjob/docs/wave-1/reviews/T26-structural.md); R29–R31 |
| Astra → integration and instruments | Scanner could report clean on non-UTF-8 or inaccessible inputs; recovery could abandon a leader-less process group. | [T26 GPT finding 1](/home/mstie/projects/taurjob/docs/wave-1/reviews/T26-gpt-review.md); [rerun findings 1–2](/home/mstie/projects/taurjob/docs/wave-1/reviews/T26-rerun-gpt-review.md); R33–R34 |
| Fable design → live integration | Queued reads appeared to be starting; partially withheld but retained reports appeared missing; stopped passes appeared as missing reads. | [T20-live findings 1–3](/home/mstie/projects/taurjob/docs/wave-1/reviews/T20-live.md); R32 |
| Opus → acceptance evidence | Required the high-volume counter-loss trigger to survive reruns, hardened privacy exemptions, and independently checked final scanner verdicts in both directions. | [T26 product acceptance](/home/mstie/projects/taurjob/docs/wave-1/reviews/T26-product-acceptance.md); R27a/R35a; [certification acceptance](/home/mstie/projects/taurjob/docs/wave-1/reviews/T26-cert-product-acceptance.md) |

This was substantive review. However, Opus’s initial T2 review cited two strings absent from the candidate and incorrectly passed forbidden lens controls. It subsequently retracted three judgments and acknowledged that Astra’s ten findings subsumed its own. This is a serious grounding failure, followed by an appropriately explicit correction. [T9 acceptance, “Corrections to my own T2 product review.”](/home/mstie/projects/taurjob/docs/wave-1/reviews/T9-acceptance.md)

Astra’s reviews of review reports also narrowed overclaims: typing into a short-name field did not prove statement persistence; an incompatible synthetic database row did not prove retry universally broken. These checks earned their cost because they inspected the candidate and constructed counterexamples. They do not justify automatically reviewing every review. [T20-setup GPT:R1; T20-results GPT:R1.]

**Decision 4:** one Opus seat was maintained and later earned its place. Its product/acceptance lens added distinct value, despite the failed first design pass. The stronger claim that Astra alone suffices for core structural review remains untested here: Fable performed most structural checkpoints in this Product Build wave. Neither removing Opus nor declaring Astra’s solo structural sufficiency follows from these results. [A: resolved launch models; L: review routes; reviews above; B: Decision 4.]

**3. The altitude seat was valuable; the supplied account of its inactivity is contradicted by primary evidence.**

The archive records altitude-reviewer completing its pre-read and waiting for `T7 go` at 19:40 and again at 19:42 UTC. Task #7 later records its start and completed review; task #1 carries its rejection rulings. The “never acknowledged a task” characterization should be removed from the incident record. Absence of an acknowledgment was particularly weak evidence because the standard prohibited pure acknowledgments. [A: `inboxes/team-lead.json`, 19:40:39 and 19:42:58; `tasks/7.json`, rulings 1–2; `tasks/1.json`, rulings 3–4; S.]

Nor does `v3-architect-codex` establish a provider mismatch. Both the archived instructions and current [role text](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/v3-architect-codex.yaml) explicitly retain that identifier for compatibility while preferring Fable. The config’s model is null, but `telemetry/7.jsonl` resolves the launch to **claude/fable/high**. Product-reviewer similarly has a null configured model and resolves to Opus. The evidence supports confusing configuration presentation and uptake visibility; it does not establish that the legacy role ID caused inactivity.

The lead’s authorized substitute altitude pass added the M3a/M3b decomposition and an owned portal-parity measurement. The resumed reviewer added privacy-failure granularity, a closed persisted payload type, narrower legacy import scope, and separation of persisted transitions from published activity. These were distinct architectural changes beyond a product review. [T1 lead altitude findings 1–2](/home/mstie/projects/taurjob/docs/wave-1/reviews/T1-altitude.md); [altitude-reviewer findings 1–4](/home/mstie/projects/taurjob/docs/wave-1/reviews/T1-altitude-reviewer.md); R6–R9; T10.]

There is an independence qualification: the resumed reviewer disclosed reading the substitute’s review before its first commit. Its additional findings have demonstrable value, but these were not two blind verdicts. The duplicated T12 pass was less useful: reassignment produced competing deliverables and a temporary deletion, with the extra Opus pass ultimately agreeing. [T1-altitude-reviewer independence disclosure; L:E10/T12.]

**4. The routing report cannot grade cost efficiency, seat utilization, or oversize attribution reliably.**

Q reports seven role/model rows, 23 accepted counts, zero oversize incidents, zero nudges/staleness, and median wall times ranging from 1m59s to 234m07s. Those are report outputs, not a validated wave-level cost comparison.

- The directly accessible task archive contains only **11 tasks and 23 ruling records**: 11 verdicts, nine notes, three other rulings. The later ledger reaches T28. Full-wave ruling and lifecycle totals cannot be reconstructed from that early task snapshot.
- Routing sidecars contain **19 launch events, nine without task IDs**, and five completion observations. Every one of the ten Astra launch events has null capability tier and rank. The lead appears only in unattributed launches and has no Q row. [A: `telemetry/*.jsonl`.]
- T9 changed from heavy-implementer-1 to judge-astra-1. Both launches are retained. Q’s role-row totals and model totals differ, and the two heavy seats are merged into one row; task and member attribution needs explicit reconciliation. [A: `tasks/9.json`, `telemetry/9.jsonl`; Q.]
- T7 and T9 completed without a verdict on their own review task, while substantive rulings were placed on the reviewed candidate. Thus `completed_unruled` can reflect where a verdict is stored, rather than absent review. [A: tasks #1, #2, #7, #9.]
- Q’s zero nudge count does not mean no nudges occurred: the archived judge inbox contains an idle-monitor nudge at 19:57:01, and the lead inbox contains two stall escalations. These may be a different event class from deadline nudges, but the report does not explain that distinction. [A: `inboxes/judge-astra-1.json`, `inboxes/lead-taurjob.json`.]

Sol’s 234m07s median and heavy Astra’s 34m15s median compare different work, including waiting, live runs, and review latency. They cannot establish a speed or cost advantage. Keep Decision 2’s lead/model experiment gate closed until task identity, scope, waiting time, and acceptance links are measurable. [Q; B: Decisions 2–3 and cost-shape prediction.]

**5. Ceremony dominated recorded activity, and the wave lacked an early stopping rule for instrument work.**

The following measurements use git range `372d8aa..1107f30` and the tree at its endpoint. They measure artifact activity, not human-equivalent labor or token cost.

| Measure | Observed result | Interpretation |
|---|---:|---|
| Wave commits | 603 | Three seed commits and later retros excluded |
| Prose/design-only commits | 466 / 603 = **77.3%** | Includes useful specifications and reviews |
| Commits touching code/tooling | 122 / 603 = **20.2%** | Includes tests and acceptance instruments, not only product |
| Coordination-only commits | 283 / 603 = **46.9%** | Paths limited to ledger, rulings, closure, recreation |
| Ledger / rulings / plan / closure revisions | 257 / 48 / 46 / 60 commits | Counts overlap; do not sum them |
| Task structure | 28 numbered tasks; ten include implementation | Remaining 18 cover review, specification, measurement, diagnosis; T6a adds a ledger row |
| Review documents / evidence files | 54 / 1,312 | Review prose totals 5,538 lines |
| Seven primary implementation slices | 14 structural checkpoint reports + eight product reports | **3.1 review documents per slice**, before design and subsequent addenda |

Adding the three surface design reports and their three GPT reviews gives **28 review documents for seven primary slices**, or four per slice. This remains a lower-bound document count, not a count of independent reviewers or all review rounds. M2 alone went through A, A2, A3, and B structural checkpoints. [Review files for T11/T15/T16/T19/T21/T22/T24 and T20.]

My defensible ceremony estimate is therefore **roughly four-fifths of commit activity**, with nearly half devoted solely to coordination documents. A wall-time percentage is unavailable. Preparation, verification, and defect fixes must not all be counted as waste.

The clearest waste was maintaining the machinery’s descriptions: repeated commits trimmed unchanged reviews to line budgets, while the ledger accumulated a **24,179-character single task row**. The closure report records an acceptance-plan cell previously reaching 21,661 characters. Line ceilings encouraged compression without controlling reading burden. [Git: `be074a2`, `16277b9`, `7ab2f7c`, `d02d5bc`, `d373ca0`, `fcb7fcd`; L at cutoff; C: process finding 12.]

Instrument scrutiny initially paid: the scanner had reproducible false-clean paths. It then expanded into eleven refinement items and repeated rubric maintenance before the operator prompted R41’s freeze. That is evidence of missing cumulative scope control, not grounds to dismiss all scanner work. The late T28-12 exception was explicitly authorized; it was not an unrecorded breach of the freeze. [R41, including item 5.]

**6. The heavy-implementer leash shaped behavior, but its rejection/telemetry mechanism was not demonstrated.**

There is positive behavioral evidence. Heavy-implementer forecast M0 at 377 handwritten lines, noticed rustfmt would bring it to 412, and obtained a 430 ceiling before landing 413. Later accepted totals were explicitly bounded: M1 1,982/2,100; pipeline core 1,630/1,700; enrichment 2,097/2,200; closure lanes 1,503/1,550 and 1,108/1,200 delivered. [A: task #11 progress and ruling; L:T11/T15/T16/T24/T27/T28.]

Checkpointing and architectural decomposition also helped, particularly M3a before M3b. That split came from altitude review; the evidence does not isolate the line budget as its cause. [T1-altitude finding 1; L: milestone slicing.]

No supplied heavy-code review records a size rejection with `field=oversize_diff`. The reviewers did inspect budgets and found accepted candidates within revised ceilings. Thus the honest finding is **“forecasting and approval worked; the failure path was not exercised in supplied evidence,”** rather than either “leash ignored” or “zero proves success.” [Heavy role; T15-B/T16-B/T24-B; T26 GPT budget paragraph; Q.]

Three weaknesses remain:

- **Budget semantics varied:** final file length, net additions, gross churn, handwritten exclusions, and consolidated task-owned changes all appear. T26-fix-1 explicitly counted edits to newly introduced lines once, while T27 reported gross batch accounting. Those can be legitimate measures, but are not interchangeable. [T15-A/B; T26-rerun GPT budget clarification; L:T27/T28.]
- **Ceilings kept expanding:** T27 reached 1,550 through repeated approvals. Authorized additions are not oversize violations, but a succession of local approvals supplied no effective overall wave budget. [L:T27; R41.]
- **Documentation overages received post-hoc acceptance:** T2a and T18 exceeded their limits. These were not heavy-code violations, so they do not explain Q’s heavy zero; they do expose inconsistent enforcement of the ledger’s broader “review failure” rule. [L: diff budgets, T18.]

**7. All nine seats eventually contributed; nine permanent seats were not justified.**

The table grades observed assignments against the named role contracts, not against model reputation. Model/effort values come from resolved launch telemetry.

| Seat | Contract match and distinct value | Staffing judgment |
|---|---|---|
| lead-taurjob — Fable/high | Routed contracts, reconciled decisions, performed authorized altitude fallback and real smokes. No demonstrated product implementation takeover. State reconciliation and repeated rule repair consumed substantial work. [L:T7, smoke log, E7/E10/E16; lead role] | Essential; needs deterministic support |
| architect — Astra/high | Delivered T1 and R15/R15a, accepted preparatory evidence, preserved structural ownership. Altitude materially revised its initial decomposition; later implementation exposed missing prompt and persistence seams. [L:T1/T14; T1-altitude; R21/R31; architect role] | Earned planning phase; dedicated full-wave seat not demonstrated |
| implementer-1 — Sol/medium | Produced CLI/parity measurements, M2 and integration. M2 A violated the end-to-end behavior contract; later corrections and integration work were substantial. [T3/T10/T19/T26; E12/E14; developer role] | Keep; give a smaller executable first slice |
| heavy-implementer — Astra/high | Toolchain measurement, scaffold, setup/import, enrichment and fixes; strong forecast discipline. Red scaffold broke master; ordinary setup and documentation also consumed this premium lane. [T4/T8/T11/T15/T22/T24/T27; E13] | Keep one heavy lane with explicit qualifying scope |
| heavy-implementer-1 — Astra/high | Pipeline inventory/core, results/persistence, withholding and cleanup fixes. Clearly productive; recorded one landing-before-gate exception. [T5/T16/T21/T28] | Capacity earned; need for a second Astra tier unproven |
| product-reviewer — Opus/high | Acceptance and adversarial evidence checks added value. Initial T2 grounding failed; late unqualified staging violated ownership repeatedly. [T9 acceptance; T26 product/certification; E16; product role] | Keep one; constrain acceptance claims and checkout access |
| altitude-reviewer — Fable/high | Architecture and structural review closely matched its actual role; caught execution facades and cross-layer defects. Initial inactivity account is unreliable. [T7/T12/T19/R31; archive pre-read messages] | Keep independent Claude structural review |
| design-taurjob — Fable/high | Delivered design and rendered critiques, then detected live state drift. Overbroad jargon rules and synthetic-evidence claims required correction; edited a candidate during review. [T2/T20; T20 GPT reports; E11; design role] | Earned; schedule by design and surface milestones |
| judge-astra-1 — Astra/high | Initially lacked the required paired cell; explicit T9 reassignment supplied a single-sided rubric. Thereafter produced valuable isolated design, code, and instrument reviews. [A: task #9 and inbox assignment; T9/T23/T26; judge role] | Keep the review capability; remove the standalone dual-judge seat |

Decision 1’s Fable-lead/Astra-architect orientation is provisionally supported: delegation held and the architecture packet was used. It was not a clean one-pass handoff, and no comparative evidence supports changing the lead model. [L:T1/T14/R21/R31; B: orientation prediction.]

For wave 2, start with **seven active seats**: lead, Astra architect transitioning into GPT reviewer, Fable structural reviewer, Opus product reviewer, design lead, one Sol implementer, and one Astra heavy implementer. Add an eighth **Sol** lane only when a third independent implementation slice is ready. This is a proposed operating trial, not a measured optimum. It preserves every demonstrated review lens while removing the separate judge assignment and default second heavy tier. [Role evidence above; B: Sol-workhorse principle.]

**8. Dependency waits were frequently mistaken for uptake problems; communication layers fought each other.**

The clearest example is T9: its assignment explicitly said review starts on `T9 go`; the idle monitor nudged at **19:57:01**, before go arrived at **19:58:00**. Canonical start followed at 19:58:23. Later T27/T28 legitimately waited between fix batches but were repeatedly nudged until owners recorded blocked state. [A: judge inbox/task #9; L: “Closure lanes.”]

The archive also exposes conflicting instructions. Onboarding said **“Acknowledge assignment, execute, then report completion…”**; generated assignments said **“Do not send an acknowledgment…”**; S prohibited pure acknowledgments. T9 received both a generated assignment and a long manual contract. These are concrete sources of duplicate reading and uncertainty about expected response. [A: heavy/judge inboxes; S.]

Wake telemetry recorded **1,262 events in about 29½ minutes**: 1,133 suppressed, 65 injected, 64 observed. Of the suppressed events, 1,025 were low-priority/empty-message suppression. These are daemon events, not 1,262 model messages; they show repetitive machinery activity, not token expenditure. [A: `state/protocol_telemetry.jsonl`.]

Two lead inbox names held different records: `team-lead.json` contained 34 notification envelopes, while `lead-taurjob.json` contained two escalations. Their unread flags do not prove nondelivery—subsequent ledger actions show some information arrived—but the split undermines straightforward uptake auditing. [A: those inboxes.]

Useful dependency management did occur: R17 resolved an unanswered type seam, R20 moved enrichment to available capacity, and R13 exempted documentation from code gates after Cargo contention. Resource failures nevertheless persisted: concurrent builds triggered memory protection, and fresh target directories consumed substantial disk. [L:R13/R17/R20, host-memory and gate-evidence rules.]

**9. Ranked adjustments — cheap, before wave 2.**

1. **Use isolated worktrees for every writing seat and serialize landing.** Gate the actual combined candidate before landing; preserve ownership through the landing record. A documentation seat should be unable to stage another lane’s edits. [Finding 1; E13–E16.]
2. **Preflight the resolved launch contract.** Show canonical repository, readable standard, effective provider/model/effort, assignment token, first action, dependencies, and expected artifact before launch. Validate effective values rather than legacy role names. [Findings 3–4; E7.]
3. **Freeze one review manifest per candidate:** code commit, rubric revision, allowed evidence, reviewer lens, completion criteria. A changed rubric produces a bounded delta review; it does not silently become “plan tip at RESULT time.” [T9 rubric skew; L:T25; T26 structural B1.]
4. **Use the seven-seat trial above and retire the paired-judge assignment.** Move the architect into independent GPT review after architecture freezes. Keep one Opus and one independent Fable structural reviewer. [Finding 7.]
5. **Separate waiting, review-ready, accepted, and closed states operationally.** A dependency wait carries the awaited artifact and owner; it suppresses idle nudges. Acceptance should close routine tasks without another member round trip. [Finding 8; A: T4/T8 closeout messages; L: overdue closures.]
6. **Cap instrument scope and document updates at wave start.** Give acceptance-tool work its own budget and stopping condition. Batch ordinary ledger changes; retain only current state plus links in the table. Generate the closure list from accepted/deferred items so already-fixed work does not remain in wave-2 prose. [R41; L:T25–T28; C: stale T28-12 and leader-less-group deferrals.]

Concrete role-text replacements:

| Contract | Exact text to change | Replacement |
|---|---|---|
| [Lead](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/v3-lead-claude.yaml) | “Use the task system as the source of truth.” | “Use the task system as canonical state only after verifying owner, assignment generation, dependency state, and accepted artifact. Escalate lifecycle contradictions; never infer inactivity from silence.” |
| [Heavy implementer](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/astra-heavy-implementer.yaml) | “Do not begin without a numeric or otherwise objectively checkable diff budget.” | “For implement work, record the baseline, owned paths, counting method, exclusions, and numeric budget before editing. Measure/diagnose work follows its assigned work-kind evidence contract.” |
| [Developer](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/v4-developer-codex.yaml) | “The behavior works end to end with real data (a placeholder only when the assignment asked for one).” | “An execution checkpoint proves command → dispatch → persisted transition with a controlled real process before UI work starts. Name incomplete stages explicitly in its RESULT.” |
| [Structural reviewer](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/v3-architect-codex.yaml) | “Flag missing coverage to the developer, not as a blocker but as a request:” | “Block when missing evidence prevents verifying a required persistence, privacy, recovery, or execution invariant; otherwise record a bounded coverage request:” |
| [Product reviewer](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/adversarial-reviewer-claude.yaml) | “Prioritize high-signal review output over broad commentary.” | “Every PASS names the exact candidate evidence and its scope. Verify quoted strings against candidate bytes; distinguish independently checked, accepted on citation, and unmeasured claims.” |
| [Design lead](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/claude-design-lead.yaml) | “Instant-fail Comprehension & Copy for version numbers, developer jargon, release-note prose, descriptions of system internals, or placeholder-quality copy in user-facing text.” | “Fail unexplained app-authored jargon that obstructs comprehension. Preserve required model output, historical identifiers, and versioned provenance; add plain-language context without rewriting source evidence.” |
| [Astra judge](/home/mstie/projects/taurhaus/src-tauri/resources/templates/roles/judge-astra.yaml) | “Judge the same material, question, rubric, evidence allowance, and stopping condition as Judge Fable.” | “Activate this paired-judge role only with both judges and a fixed shared cell manifest. Ordinary product or code review uses a reviewer role with its own named rubric and stopping condition.” |

Apply the same changes to compact summaries and onboarding. Specifically replace onboarding’s acknowledgment instruction with **“Execute the first action; report through the named completion signal. Record an explicit dependency wait when execution cannot begin.”** [A: onboarding conflict; S.]

**10. Structural adjustments — give these dedicated machinery lanes.**

- **Transactional lifecycle enforcement:** validate owner and assignment generation on accept/start/progress/review/complete; reject stale operations; prevent accidental reopening; record accepted artifact and review task as linked objects. Acceptance test: replay E10 and the reopened #13 sequence. [L:E10, board reconciliation.]
- **Safe workspace lifecycle:** prevent moving/removing active working directories; support an explicit relocation transaction that pauses members, updates assignments and paths, and verifies restart. Add landing ownership and build-slot controls. [Recreation; E7/E13–E16; resource contention.]
- **Auditable routing telemetry:** immutable team/wave/member/task IDs, old/new owner, effective model and tier, budget revisions, candidate hash, reviewer identity, accepted artifact, and active/waiting/review durations. Separate idle-monitor and deadline events. Prove an induced oversize rejection appears once on the implementer’s row, not the reviewer’s. [Finding 4; B: Decision 3.]
- **Shared evidence runner:** build identity, isolated data/display/port, process ownership, source-shaped fixtures, negative controls, and an immutable evidence manifest. Validate count → verdict wiring before large certification runs. [E15; T20 GPT reports; T26 scanner findings.]
- **Generated operating views:** derive current ledger, review queue, closure report, and wave-2 carryovers from linked task/ruling data; preserve history separately. This targets the 257 ledger revisions and unreadable task rows directly. [Finding 5.]

**Executive summary — ten lines**

1. The team delivered through real defect detection, but people repeatedly repaired machinery failures. [L:E7–E16]
2. Fix ownership, isolated landing, and lifecycle enforcement before adding more seats or rules. [E10/E13–E16]
3. The altitude reviewer contributed; “never acknowledged” contradicts archived pre-read and review records. [A:T7/inboxes]
4. Keep complementary product and structural lenses: they caught materially different defects. [T19/T20/T26 reviews]
5. Keep one Opus seat; its first design pass failed grounding, while later acceptance work earned its place. [T9/T26]
6. The heavy leash influenced forecasts and approvals; its oversize rejection and attribution path remains unproven. [T11; Q]
7. Ceremony occupied 77.3% of commit activity; labor-time share cannot be inferred from that measure. [Git audit range]
8. Nine permanent seats were not justified; trial seven, adding a third implementation lane only for ready work. [Seat audit]
9. Q cannot support Astra-versus-Sol cost claims until scope, waiting, ownership, and acceptance are linked. [A; Q]
10. Bound acceptance-instrument work at kickoff so another operator-triggered late freeze is unnecessary. [R41]
