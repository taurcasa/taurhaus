## Run 4 execution — UNAVAILABLE: step-1 harness race, no paid retry

This is the latest run's verdict. The earlier run-4 pre-ruling admission refusal is superseded: the orchestrator explicitly closed runs 1–3 and authorized fresh caps of 8 Claude inputs (including compact), 12 Codex inputs, $2, and 15 minutes. The ruling is quoted verbatim in `admission-ledger.json`; the prior 8/11 input ledger remains unchanged in `prior-spend.json` and embedded history. No budget clarification is pending.

| Ordered step | Outcome and classification |
|---|---|
| 1. Initialize, signed-in/team-bound lead, owner and hooks | **FAIL — harness**: immediate session-ID assertion before scanner attribution; overall lane **UNAVAILABLE**. |
| 2. Claude sends both distinct markers to alpha | **NOT RUN** — dependency on step 1. |
| 3. Alpha reads/replies; native Claude uptake and distinguishing response | **NOT RUN** — dependency on step 1. |
| 4. Claude explicitly reads/marks its inbox with cursors | **NOT RUN** — dependency on step 1. |
| 5. Ordinary `/compact`, genuine hook and recovery card | **NOT RUN** — dependency on step 1. |
| 6. Fresh native reply after recovery; export | **NOT RUN** — dependency on step 1; failure evidence exported and teardown completed. |

### Exact stop and partial observations

`step1-driver.log` points to the executed `step1.py:23`: `assert lead['session_id'] and lead['paneId']=='%1'`. The retained `step1-observation-end/team/runtime/lead.json` has pane `%1`, attachment 1, context generation `0`, but null session identity. The driver asserted immediately instead of allowing the required 60-second scanner opportunity. Runtime was **6.333125 seconds**. This is insufficient observation to diagnose a Taurhaus attribution product defect.

The raw `result.json` says `claude_auth_unavailable` because `run3_driver.py` mapped every step-1 error to that fallback. That is **unsupported**; `final-disposition.json` supersedes the classification without rewriting raw runtime evidence. The Claude pane shows **Haiku 4.5 · Claude Max**, and `claude-transcript.json` contains a native startup recovery-card input at **2026-09-10 16:37:42.868 UTC**, team `l1-claude-lead`, agent `lead`, session `e0d73e8d-b9b5-4998-98a8-f9828b526724`. No assistant generation completed, so authenticated model completion remains unverified. This was not an observed login refusal.

`initialize-result.json` records one canonical initialize, ID `init_234aab592ca34c9dae7d2541ecc37dcd`, all nine pipeline steps succeeded, format 2, delivery owner `team`, two managed launch-new tmux seats. Production `SessionStart(compact)` registration is retained in `production-hooks-before-launch.json`. No descriptor was edited and no hosted seat was created; the authorized variant uses two alpha markers in steps 2–3.

The complete captured journal has six rows. Lead startup message `c0ae4bd6-b0e9-4c8e-89db-5945b1fd06cd`, delivery `060936ad-5886-4e48-875d-a0f2b3eefec7`, has `native-mailbox/1` attempt and `native_enqueued` receipt (sequences 3–4). The startup card then appears in Claude's native input without any `mesh read`. `inboxes/lead.json` observations are preserved in snapshots; an empty post-poll projection is not proof of failed uptake. Alpha startup message `d3a75483-29f4-4333-b8e4-288c83ebb762` has tmux `submitted` receipt (sequence 6). These are startup observations, **not** the two required reply markers, explicit reads, model actions, or a post-compaction recovery card. Alpha attribution/idle/card completion, native reply routing after the membership fix, and steps 2–6 remain unverified. No external reader marked the lead inbox; no paginated journal export was reached.

### Provenance, costs and cleanup

Executed controller commit **bbe8fdc4**; Taurhaus merged base **1db4f9bf**, protocol **27**, version **0.9.7**. Checkout-local daemon SHA256 `9dfc9daacbe21380405cec063c37a433becfdb32b2fffffd20813879f679f590`. Designated Mesh worktree detached at **ed59187**, version string **mesh 0.2.29**, runtime SHA256 `35b1caf1e6c2379550a79c7c6e024de128af7f6b5b73f59d14ad32d23f63ca17`. Both `just build-daemon` and `cargo build --bin mesh` exited **0**, following bounded Cargo admission polling; production hook adapter link exited **0**. No binary was installed.

Actual Claude **2.1.267**, model **claude-haiku-4-5-20251001**; Codex **0.153.4**, model **gpt-5.6-luna**, effort **low**. Both native Codex siblings were resolved from the installed launcher and copied together. The confirmed-submission logic was retained, but no controller-typed model submission was reached. Full binary digests and sanitized exact commands/RPC results are in `events.jsonl` and `isolation.json`.

| Spend item | Observed amount |
|---|---|
| Claude startup, 1 reserved input / native card input | **Unknown**: interrupted before any assistant usage generation. |
| Codex startup, 1 reserved input; turn `01a08c2e-ab5b-7873-bb59-518ebb12df15` | **Unknown**: one started turn, no retained token-usage row. |
| Controller-typed inputs / compactions | **0 / 0**; no compact was attempted. |
| Run-4 total seat spend | **Unknown**, not $0; the $2 dollar bound is unverified. |
| Historical closed-run seat estimate | **$0.10193487**; not charged to run-4 admission. |
| Historical completed reviewer estimate | **$0.798975**, plus an earlier timed-out review's unknown usage. |
| Run-4 independent Opus review | **$0.111810**, one completed request; separate from the seat cap. |
| Implementer/orchestrator | Unavailable to this controller. |

`cost-ledger.json` preserves zero arithmetic counters **with `metering_complete: false`**. These zero counters do not mean free turns. The initial two onboarding starts were reserved; no subsequent paid input was admitted, and there was no retry or reset after this execution. Teardown was immediate on the harness failure and did not retain completed startup usage. The transcript-based cap cannot be certified retrospectively.

Runtime HOME, all harness roots, project/data/temp roots, private tmux socket, and probed non-default daemon port were scratch-only. Bubblewrap hid operator homes and mounted the single authorized Claude credential read-only; it was never copied. Only the authorized Codex auth file was copied, mode 0600. The complete **42-row daemon JSONL** is retained in `taurhaus.log.jsonl`; no account-usage row occurred. Bounded panes, native transcript, Codex session/turn metadata, runtime/config/activity snapshots, receipts and passive locks are retained and deduplicated after shutdown.

Controller and driver exited **2**; step-1 driver **1**. The owned Bubblewrap bootstrap exited **-15** during normal teardown. `cleanup.json` and independent `teardown-audit.json` show **zero surviving trial processes**, closed private listener, removed scratch root/Codex auth, and zero-byte Claude mount placeholder. No operator process was killed.

### Offline correction and deviations

Three new offline regressions observed red before implementation: fresh-budget reset helper missing (1 error), then missing attribution-wait and failure-classification helpers (2 errors). **35 offline tests pass** after fixes (31 discovered and four existing controller regressions). Generated fixtures invoke no real CLI or credential path. The immediate identity assertion originated in **a94047bc**; the blanket authentication fallback in **b1ca7979**. The offline correction **ccc62453** waits 65 seconds for attribution and no longer diagnoses auth from a generic driver error. It was **not used in this paid run** and does not change its outcome. No paid rerun is authorized by this report.

The exact gates ran in credential-free namespaces from the checkout root: **`just check-quick` 0**, **`just lint` 0**, **`just test-contracts` 0**. Check-quick passed 2,518 frontend tests with zero typecheck errors/warnings; contracts passed 15 renderer, 20 harness-conformance and 33 module-boundary tests. The gate wrapper exited 0. See `execution-gate-audit.json` and the retained gate logs. No `src-tauri/` file changed, so conditional `just test-rust-unit` is inapplicable. The independent **claude-opus-4-6** evidence lens exited **0**, verdict **approve**, one minor explicitly accepting the transparent unknown-spend disclosure with no fix required. One request cost **$0.111810** (2 input, 10,965 cache-creation, 86 output tokens). It approves the stopped-run report, not missing runtime steps. Its scratch namespace was waited and removed; the credential placeholder remained zero bytes. Independent process audits found no runtime, build, gate or reviewer survivor. Known closed-run seats plus completed historical/current reviews total **$1.01271987**, excluding this run’s unknown seat spend, the earlier timed-out review’s unknown spend and unavailable implementer/orchestrator usage.

Deviations: step 1 lacked its required attribution observation window; the raw driver failure label was inaccurate; interrupted startup metering cannot certify the dollar bound. Those defects make this lane unavailable. Steps 2–6, completed signed-in generation, alpha fresh-idle attribution, native marker round trip and compaction recovery remain untested. The authorized no-beta and credential-source exceptions are followed. No product change, descriptor edit, plan-ledger edit, unrelated checkout mutation, or paid retry occurred. No numbered runtime step was green; commits record preparation, offline tests and stopped-run evidence only.


Evidence packaging: `snapshots.json` maps **206 original paths to 88 unique objects**, including `initialize/`, `step1-observation-end/`, `latest/`, `final/`, build/gate logs and the Opus review. References to files in those directories are snapshot aliases. Top-level runtime files, including the full daemon JSONL and step-1 driver log, remain direct files. The earlier pre-ruling gate records and disposition are retained as `pre-ruling-gates/` aliases and `pre-ruling-final-disposition.json`; closed-run ledgers were not changed. `execution-gate-audit.json` supersedes the historical gate audit for this paid execution.
