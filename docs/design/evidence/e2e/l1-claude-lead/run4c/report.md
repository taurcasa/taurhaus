superseded_by: run4d

Original FAIL/taurhaus classification below is retracted: review established UNAVAILABLE — harness readiness gate. See run4d for the latest real execution.

# Run 4c — FAIL at step 2: Taurhaus activity export never freshly idle

Run 4c actually executed on 2026-09-10, 16:59–17:00 UTC, using the repaired attribution-wait controller (executed commit `35a82a7d`). This verdict supersedes earlier attempts; their evidence and unknown spend remain historical. No product change or paid retry was made.

| Ordered step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize; authenticated, team-bound Claude; owner and hook registration | **PASS** | `initialize-result.json`, `step1-outcome.json`, `production-hooks-before-launch.json`, `claude-transcript.json` |
| 2. Claude sends two distinct markers to alpha | **FAIL — taurhaus**, at the prerequisite activity check, before either send | `step2-outcome.json`, `step2-window-0.json`, `step2-failure/team/state/activity/alpha.json`, `runtime-session-snapshot.json` |
| 3. Alpha explicitly reads/replies; Claude natively receives and distinguishes both markers | **NOT RUN**, dependency on step 2 | `driver-steps.json` |
| 4. Claude explicitly reads/marks with cursor pagination | **NOT RUN**, dependency on step 2 | `driver-steps.json` |
| 5. Ordinary `/compact`, real hook boundary and recovery card | **NOT RUN**, dependency on step 2 | No compaction input or post-compact generation |
| 6. Fresh reply after recovery and native action | **NOT RUN**, dependency on step 2 | Failure evidence exported; teardown completed |

The initialize ID is `init_94bc364c2e274ad0b37fed1598e90959`; all nine production initialization steps completed once, format 2 and delivery owner `team`. Lead session `02c52f09-821d-4f44-bb5b-4d0b9c32ca37`, pane `%1`, consumed its native startup recovery card and completed an authenticated Haiku answer at 16:59:13.724 UTC: “READY” followed by a request for a task. The production `SessionStart(compact)` hook was installed before launch. This proves startup, not compaction recovery.

Alpha session `01a08c42-3cf9-7be0-8831-7015c4de9d08`, pane `%2`, has an attributed rollout path. Its startup turn `01a08c42-4218-7990-b26a-a65eb7c0c547` completed at 16:59:31.246 UTC. The pane has an empty composer and final response. The 65-second prerequisite window began at 16:59:16.921 UTC with 886 seconds of outer headroom. Exported alpha activity was `likely_working`, then **`uncertain`** at 17:00:13.337 UTC (42 seconds since output), never the required `idle`. The final runtime snapshot independently reports `state: idle`, `activity_confidence: low`, `activity_attribution: none`. Identity attribution and activity attribution are separate facts.

The exact raw stop is `taurhaus: alpha not attributed and freshly idle after #163`. The **identity portion succeeded**; the failed conjunct is freshly idle activity export. Route this to Taurhaus activity classification/export, not missing session attribution, Mesh mailbox projection, or Claude authentication. `taurhaus.log.jsonl:142` records Codex active→idle from source `none` at 16:59:41.832 UTC. Read-only source inspection of `src-tauri/src/coordination/activity_export.rs:585` shows that an alive non-shell process with idle/low/unattributed activity can export `uncertain`; root cause beyond this observed mismatch is not claimed. No product fix was attempted.

Alpha's onboarding card **did reach the model**: message `b119ad03-51be-4f19-8aa1-509ade0cb315`, delivery `fa3332fb-99ec-44c9-8ffd-70d2929ae28a`, has tmux `submitted` at sequence 6, then `consumed_by_read`, reader alpha, context `explicit-mesh-cli` at sequence 7. Its transcript contains the actual recovery card returned by Mesh. The card is not pending for lack of exposure. Alpha then deviated from the trial's no-unsolicited-send instruction: it attempted four invalid send commands while asking for task context; all returned errors, with no new accepted message. These were tool continuations within its one metered startup turn, not controller inputs or retries. They are retained in the transcript and pane, not counted as step-3 replies.

Lead startup message `db2c4b99-98eb-4174-b55f-47e5b124524c` is identified authoritatively in `review-excerpts.json` (journal sequences 1, 3–4). Its `native-mailbox/1` receipt is `native_enqueued`; the native Claude transcript independently confirms startup uptake. `inboxes/lead.json` exists and the final projection is empty after native polling. No lead `mesh read`, lead terminal-delivery receipt, reply markers, or explicit lead consumption receipt occurred. No external reader marked the lead inbox. Journal CLI pagination was not reached; all seven complete journal rows are retained from the scratch files. Startup evidence does not establish the requested native round trip.

## Builds, metering and isolation

Taurhaus base `1db4f9bf`, actual daemon protocol **27**, version **0.9.7**; checkout-local `just build-daemon` **0**. Mesh was detached at `ed59187` in the designated separate `mesh-l1` worktree; `cargo build --bin mesh` **0**, descriptor unchanged. Production hook adapter build **0**. Exact commands, bounded Cargo admission polls and binary SHA256 values are retained in `gates/gate-*-build.json`, `hook-build.json`, `events.jsonl` and `executed-source.json`. Actual Claude **2.1.267**, model **claude-haiku-4-5-20251001**; actual Codex **0.153.4**, **gpt-5.6-luna**, effort **low**. Both native Codex siblings were copied into scratch bin. No install, hosted seat, beta, or descriptor override was used.

| Spend | USD / accounting |
|---|---|
| Run 4c Claude: one startup input, one metered generation | **0.01179540** |
| Run 4c Codex: one startup input, one completed turn | **0.00511548** |
| Run 4c seats total | **0.01691088**; conservative all-token upper-rate estimate **0.14121140** |
| Claude cap × maximum observed input | **0.09436320**, below $2 |
| Controller-typed inputs / compact | **0 / 0**; confirmed-submission mechanics retained but not reached |
| Previous run-4 attempt's interrupted startup | **Unknown**, retained as history; offline continuation new spend **0** |
| Closed runs 1–3 seat estimate | **0.10193487**, history only |
| Previously completed independent reviews | **0.910785**, plus earlier timed-out review **unknown** |
| Current independent Opus review | **0.164920** (one completed request; separate $0.20 cap) |
| Implementer/orchestrator spend | Unavailable to the controller |

The standing budget rule explicitly grants **every attempt** fresh caps of 8 Claude inputs including compact, 12 Codex inputs and $2 metered spend. `admission-ledger.json` quotes both binding rulings, starts at zero, and embeds prior history without charging it to this attempt. Current usage is 1/8 Claude and 1/12 Codex, metering complete. No budget question or retrospective zero-cost claim was made. Rates are the controller's retained estimates from transcript usage, not an invoice.

Bubblewrap hid operator homes; HOME, harness roots, project, data, temp and private tmux were scratch-only, inherited TMUX removed. The explicit Claude credential was mounted read-only as a single file and never copied; only the authorized Codex auth file was copied, mode 0600. No alternative credential source was searched. Runtime was **81.524 seconds**. The complete **226-row daemon JSONL** is retained, with no account-usage rows excluded; transcript usage, runtime/activity records, seven journal rows and bounded panes are retained. No fault injection or stress work occurred.

Driver/controller exited **2**; step 1 exited **0**, step 2 exited **1**. All owned children were waited; the bootstrap exited **-15** on teardown. `cleanup.json` and the independent `teardown-audit.json` verify zero surviving trial processes, closed private listener, removed scratch root/Codex auth and a zero-byte Claude mount placeholder. No unowned process was killed.

## Verification and deviations

The run4c-routing regression failed first at the old label assertion, then all **33 discovered offline tests passed**; four existing controller regressions also passed (**37 total**). A second generated test proves an earlier attempt's unknown spend remains history while current admission enforces its fresh cap. Tests use fixtures and do not execute CLIs or access real harness homes. Only evidence routing was adapted; the repaired runtime controller and attribution wait were reused.

Exact gates ran in a credential-free namespace from the checkout root: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**. Wrapper exit 0; all gate/build children waited and scratch roots removed, independent process audit zero survivors. See `gate-audit.json` and `verification/gates/`. No `src-tauri/` diff, so conditional `just test-rust-unit` is inapplicable. Independent **claude-opus-4-6** evidence review exited **0**, verdict **approve**, two minor findings already resolved/accepted (activity-wording correction and historical unknown spend). No fix round was required. Review usage: 2 input, 16,151 cache-creation, 136 output tokens; current seats plus review **$0.18183088**. Review scratch root removed, zero-byte credential placeholder, independent survivor audit empty. The reviewer saw gates pending; the subsequent gate results were independently checked. See `review-summary.json`. This report claims a stopped product-failure run, never an E2E PASS.

Deviations/limits: step 2 failed before the requested marker sends, so steps 3–6 and native reply/compaction behavior remain unverified. Alpha attempted unsolicited invalid sends during startup; no accepted reply resulted. The initial progress message described activity as staying `likely_working`; retained final evidence corrects that to `uncertain`. The authorized no-beta variant and credential-source exceptions were followed. No product, plan-ledger, branch, or descriptor edits. Step 1 was committed immediately; failed-step evidence is committed after review and verification.

Archive audit: **301 snapshot aliases / 99 unique objects**, including build/gate and review files. Top-level transcripts, journal excerpts and the complete daemon JSONL remain directly accessible. The initial path scan falsely matched `/home` inside a scratch `/tmp/.../home` path; the corrected standalone-path check passed with no evidence redaction needed. `archive-audit.json` records this validation correction, bounded panes and the daemon-log digest.
