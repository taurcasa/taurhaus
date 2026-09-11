# Run 5 — UNAVAILABLE at step 5 (harness)

**S-runtime: steps 1–4 PASS; step 5 UNAVAILABLE — harness; step 6 NOT RUN.** Run5 proves the requested post-onboarding Codex attribution and native Claude mailbox round trip. It does not prove compaction recovery or a complete attempt dollar cap. No product change or paid retry occurred.

| Step | Outcome and retained proof |
|---|---|
| 1 | PASS. One production `coordination.initialize_team`, format 2, team delivery owner, managed signed-in Claude lead, alpha tmux; production SessionStart(compact) hook registered. `initialize-result.json`, `production-hooks-before-launch.json`, `step1-outcome.json`. |
| 2 | PASS. Claude Bash sends AMBER and BIRCH; alpha remains attributed to its own session after its first turn, with notify-sourced idle. Both sends have submitted receipts and explicit-read tool-result exposure. `review-excerpts.json`, `alpha-activity-observations.jsonl`, `codex-transcript.json`, `step2-outcome.json`. |
| 3 | PASS. Alpha explicitly reads and sends both replies. Claude receives both as native teammate messages and distinguishes them before any explicit lead read. `step3-before-read/`, native transcript row 13 and response row 15, journal 16–21. |
| 4 | PASS. Claude executes the explicit read/mark; journal 23–24 separately proves `consumed_by_read`, context `explicit-mesh-cli`. The read returned `done=true`; no further cursor page was required. `step4/`, `claude-transcript.json`. |
| 5 | UNAVAILABLE — harness. Real `/compact` boundary and hook received/resolved, but hook skips `no_resumable_task_context`; runtime generation remains 0. Controller then aborts on a false input count before its full observation window. `result.json`, `budget-stop-reproduction.json`, `review-excerpts.json`, `final/`. |
| 6 | NOT RUN — dependency on step 5. No fresh FERN input, recovery uptake, or post-recovery exactly-once claim. `driver-steps.json`. |

Directory references resolve through `snapshots.json` after deduplication. The complete 338-row `taurhaus.log.jsonl` stays a direct file (zero excluded usage rows). Runtime duration: **109.274 seconds**, driver/controller exits **1/1**, ordered driver exits **0, 0, 0, 0, 1**. Each passing step has its own commit.

## Identity and mailbox evidence

Alpha session **01a08d4d-6eb8-7700-b6b5-8c7ae4f11ce6** stays bound to its rollout and writer-lock name. A foreign ephemeral notify for **01a08d4d-758f-7580-b505-67a7bc87379b** occurs at 21:51:02.426 UTC. Alpha's own first completion arrives at 21:51:30.831 UTC; the scanner records **active → idle, source notify** at **21:51:31.268 UTC**, retaining alpha's ID. Further notify-idle transitions follow both subsequent turns. Passive samples and `codex-session-files.jsonl` retain runtime, activity and lock/rollout names; no post-first-turn unattribution was observed.

The two outbound message IDs are **a11b2e08-4965-4125-80ff-e5cdde74a81e** and **787abdaa-b145-49f2-afa8-76bcdf11833f** (journal 8–9). Receipts 12–13 are `submitted`, 14–15 `consumed_by_read` by alpha. Marker text appears in actual Codex tool results, beyond mere prompt/acceptance presence.

Replies **f45b809d-2717-4f38-b132-7f514b0518c3** and **ba2147c9-0ec7-47dd-b4e7-d31913b53502** are accepted at 16–17, then `native_enqueued` via `native-mailbox/1` at 20–21. Claude session **c42a4d9d-0c53-4298-b6f5-6b3cf6774f4d** receives both in one native teammate batch and responds: “Markers confirmed: L1_AMBER_42 and L1_BIRCH_73 received by alpha.” `inboxes/lead.json` exists and is empty by the pre-explicit-read snapshot; native transcript plus projection receipts prove uptake, without inferring a canonical read receipt from it. Later explicit-read receipts are separate. No terminal-delivery receipt for Claude was observed, no drain descriptor was enabled, and no external reader marked the lead inbox before step 3.

## Step 5 boundary and classification

The controller reserved **4 Claude inputs**: onboarding, step-2 send request, step-4 read request and the one compact. Its retained ledger incorrectly reports **9**, because `cumulative()` takes the maximum of controller inputs and every string-valued transcript user row. The compact adds a summary, local-command caveat, command echo and local-command output; the native reply batch is counted too. The false cap stop happened about 28 seconds after compact input, before the required 120-second observation ended. This is a harness stop, not proof of a product timeout or actual input-cap exceedance. The unchanged runtime controller is retained, with an offline evidence reproduction; this lane makes no repair or rerun.

At 21:52:43.118–.127 UTC, the real hook received `source=compact`, resolved lead, and skipped **no_resumable_task_context**. The operational snapshot has empty task ID/status and role ID. `compact_hook.rs:698` explicitly skips snapshots without resumable task context. Therefore this fixture cannot establish a recovery failure for a properly assigned seat. The summary's old recovery-card wording is not a new hook card. Context generation and card revision remain unchanged.

## Spend, isolation and verification

Known Claude usage **$0.03495185**, Codex **$0.01284780**, known seats **$0.04779965**; retained upper-rate amount **$0.60329760**. Six ordinary Claude API generations, three Codex native turns, four reserved Codex inputs, one compact. The compaction summarizer has no separate usage-bearing generation in the retained transcript: **compact spend and complete attempt spend are unknown**, not zero. The controller's `metering_complete=true` only establishes its Codex rollout accounting and does not prove complete compact metering. See `spend-summary.json` for current review spend and all historical amounts.

Historical seats, excluded from this fresh attempt: runs 1–3 **$0.10193487**, run4c **$0.01691088**, run4d **$0.02511108**, interrupted run4 startup unknown. Earlier completed reviews **$1.419500**, earlier timed-out review unknown. Implementer/orchestrator usage unavailable. Independent Opus review is recorded separately; no total-program spend claim.

Built from required Taurhaus base **a7e6db7e**, protocol **27**, checkout-local target; Mesh **310144d**, designated mesh-l1 only. `build-identities.json` retains binary SHA-256 digests. Actual Claude **2.1.267 / claude-haiku-4-5-20251001**; Codex **0.153.4 / gpt-5.6-luna / low**, both native siblings. No hosted seat, descriptor edit, installation or release. Startup composer/model footer observed. Typed Codex step-3 input confirmed by empty composer plus new turn before its 65-second opportunity.

Build exits: daemon **0**, Mesh **0**, production hook adapter **0**. Exact checkout-root gates in credential-free namespaces: **`just check-quick` 0; `just lint` 0; `just test-contracts` 0**. No `src-tauri/` diff; conditional `just test-rust-unit` inapplicable. Offline setup checks observed red (four setup failures, then hook-admission failure), then green; final Python test totals are recorded in `offline-verification.json`. No tests invoked real CLIs or real harness homes.

Runtime `cleanup.json` and independent `teardown-audit.json`: **zero survivors**, private port closed, scratch root and Codex auth copy removed; Claude credential only a read-only single-file bind, zero-byte placeholder after unmount, never copied. SIGTERM/-15 is ordinary owned-namespace cleanup, not fault injection. Gate/build/review children are waited and their scratch roots removed. Full scope deviations are listed in `final-disposition.json`.


Independent **claude-opus-4-6** evidence review exited **0**, **approve**, no required fix. One request cost **$0.156075**, separately from seats. Known current seats plus review **$0.20387465**, **plus unknown compact spend**; no complete total or $2-cap proof is claimed. Review credential placeholder remained zero bytes and its scratch root was removed. Final offline verification: **41 discovered tests + 11 controller regressions = 52 passed**, both exits 0. No runtime logic fix or paid rerun followed the step-5 stop.

Final archive audit: **714 snapshot paths → 202 unique objects**, all **338 daemon JSONL rows retained**. No forbidden operator paths, credential patterns or oversized pane captures were found. Independent final process audit found **zero survivors** across runtime, build, gates and reviewer; all four scratch roots are removed. See `final-audit.json`.


### Run5 continuation verification — 2026-09-11

The tree at `8794545f` was clean; passing runtime steps 1–4 were already committed individually. The requested gates ran again from this checkout root inside the credential-free namespace: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**; wrapper **0**. Offline tests: **41 discovered + 11 controller regressions**, exits **0/0**. No Rust diff, so `just test-rust-unit` remains inapplicable. All owned gate children were waited; an independent audit found zero survivors and the scratch root removed. Gate logs are retained under `run5/continuation-check/snapshots.json`, with results in `continuation.json`.

No green runtime step remained uncommitted. **Step 5 remains UNAVAILABLE — harness; step 6 NOT RUN.** Mandatory teardown removed the original sessions, and the binding spec freezes the runtime controller and requires stopping at the first failure. This continuation does not change the controller/fixture or launch a replacement attempt. New seat/reviewer spend **$0/$0**; implementer usage unavailable. Prior compact spend remains unknown. The existing Opus approval covers the stopped-run report; no new reviewer invocation or runtime success is claimed.
