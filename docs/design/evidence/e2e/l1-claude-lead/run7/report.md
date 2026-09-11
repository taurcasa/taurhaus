# Run7 — step 1 PASS; step 2 UNAVAILABLE (harness); steps 3–6 NOT RUN

Run7 executed once on Taurhaus base `a7e6db7e`, protocol 27, and Mesh `310144d`. Runtime was 90.540 seconds. The required first-failure stop occurred before the marker sends. No operational publication, assignment, `/compact`, or FERN input ran. The requested app-publication addition is implemented and offline-tested, but **not exercised live**. This is not an E2E PASS.

| Step | Outcome / classification | Evidence |
|---|---|---|
| 1. Initialize; authenticated and team-bound Claude; production hook | PASS | `step1-outcome.json`, initialization/config/runtime snapshots, native Haiku generation, SessionStart(compact) hook. |
| 2. Claude marker sends and recipient exposure | UNAVAILABLE — harness prerequisite | `step2-native-ready/pane-2.txt`, `submission-step2-2-*`, transcript and journal. Onboarding card was not read during 65 seconds; no marker sends ran. |
| 3. Seat reads/replies and native Claude uptake | NOT RUN — dependency on 2 | No AMBER/BIRCH replies or native round trip claimed. |
| 4. Explicit Claude read/mark | NOT RUN — dependency on 2 | No lead read receipt. |
| 5. Real assignment publication and ordinary compact | NOT RUN — dependency on 2 | Zero publication requests, assignment commands, compactions, or compact hook deliveries. |
| 6. FERN after recovery | NOT RUN — dependency on 2 | Zero FERN sends; export/cleanup performed separately. |

## Observed failure and limits of attribution

The raw controller recorded `FAIL / taurhaus / alpha onboarding card never delivered`. That automatic pending-card classification is retained unchanged in `result.json`; the final evidence classification is **harness unavailable**, because the inherited controller altered the pending onboarding input and explicitly prohibited the read its own gate required. This does not exonerate the preceding product transport behavior or prove the controller caused that earlier event.

Journal sequence 2 accepted alpha's baseline card: message `70337135-035a-40b3-856e-9d1a1ede7e5e`, delivery `c751f8e6-ca11-49b3-a9cb-b9e9b6c00480`. Sequence 3 began the tmux attempt; sequence 4 at 23:11:54.110 UTC recorded `outcome_unknown`, evidence `paste may be present; post-wait validation failed; submit withheld`. This was **before** the controller's input, not caused by it. There is no submitted or consumed/read receipt for alpha's card. Initial transport ownership remains unresolved; no product fix or broader product verdict is made.

At 23:12:06.404 UTC `prepare_alpha` found no `task_started` and typed its existing setup instruction without checking for pending composer text. The retained pane/native user row concatenates Mesh's inbox notification and `alphaController setup check. Reply READY only. Do not execute tools, read messages, or send messages.` Codex replied READY and performed no tools. Submission confirmation was 23:12:08.751 UTC; the subsequent card-exposure window allowed the full 65 seconds. The controller then waited for card text in tool results despite having forbidden its read. No retry or paid replacement run followed.

Alpha retained its own session `01a08d97-88aa-7171-80d1-d59861a7af66`, attributed high-confidence idle, with `source: notify` in the exported activity. Its completion notify and a foreign ephemeral notify are both retained. The early `attributed_idle: false` observation predates complete runtime identity/path capture; the final independent runtime-session snapshot shows the attribution. No decay to uncertain/source-none is established in this run. The daemon's edge log shows active transitions and launch-ready idle; there is **no logged notify-to-idle transition**, so the full PR #172 acceptance claim remains unproved here.

Lead `inboxes/lead.json` exists. Its startup card has `native_enqueued` from `native-mailbox/1`; no terminal receipt targets Claude. This is startup projection evidence, not step-3 replies or post-compact recovery evidence. All six journal rows are retained as the complete segment snapshot; the early stop reached no paginated journal-reader step.

## Run7 controller addition and red/green

The two new generated-data tests failed before implementation: missing publication method and rejected run7 route (exit 1). Green: 48 discovered tests plus 11 controller regressions, both exit 0. Tests use generated scratch task data and a mocked RPC, never credentials or real CLIs.

The real Mesh assignment description gains five explicit operational footer lines: execution mode, file-ownership boundary, adjacent-fix policy, validation expectation and response expectation. Its original objective/deliverable/first-action/completion/review contract remains. The controller reads the persisted task file after assignment, verifies assignment identity/owner/status, copies footer values and id/subject/status/owner/assigned_at verbatim, and prepares one production `coordination.publish_operational_snapshots` request for lead. The daemon remains the snapshot writer and the native hook remains the sole recovery-card producer. The controller records the task and exact request/response, then the unchanged snapshot poll precedes compact. No recovery snapshot/card/hook payload is synthesized.

The wire task includes `owner` as required by the run7 ruling; the current Rust `OperationalTaskSnapshot` does not store an owner field. This serde omission does not alter the outbound request and was not exercised in this stopped run. Footer text lives in the real assignment before publication, rather than being invented during publication. Run6 evidence and historical ledgers are unchanged. The only runtime adaptation is this addition plus run7 routing; no product/descriptor edit, hosted seat, install, release, fault injection or alternate checkout.

## Spend, builds, gates and teardown

| Current attempt spend | USD |
|---|---:|
| Claude generation `msg_011Cevc2vNS3W2Bvf4657tEX` | 0.01114540 |
| Codex sole metered native turn | 0.00193560 |
| Known seats | 0.01308100 |
| All-token upper-rate amount | 0.07127460 |

Controller reservations: Claude **1/8** (startup, zero typed Claude inputs), Codex **2/12** (startup plus one typed setup input); one actual metered Codex turn. Claude cap × largest observed input: $0.08916320; next-turn reserve $0.50. Zero compactions, so compact spend is not applicable. Foreign ephemeral notify usage is separately **unknown**, not free; the controller's metering-complete flag only covers retained rollouts. No complete attempt-dollar proof is claimed. Earlier runs are history, excluded from this fresh admission budget; `admission-ledger.json` and `spend-summary.json` retain them. Implementer/orchestrator usage is unavailable. Reviewer spend is separate and recorded below after the independent evidence lens.

Build exits: `just build-daemon` **0**, lane-local `cargo build --bin mesh` **0**, production hook adapter **0**. Binary digests are in build results and `events.jsonl`; actual Claude 2.1.267 / claude-haiku-4-5-20251001, Codex 0.153.4 / gpt-5.6-luna / low, with both native siblings copied together. Cargo admission followed the three-process limit and used checkout-local targets.

Required gates run after runtime teardown, from this checkout root inside the credential-free namespace: **`just check-quick` 0; `just lint` 0; `just test-contracts` 0** (wrapper 0). No `src-tauri/` diff; conditional `just test-rust-unit` does not apply.

Runtime cleanup confirmed zero survivors, private listener closed, scratch root and Codex auth copy removed, Claude mount placeholder zero bytes. Claude auth was exposed only by a single read-only bind, never copied. All started children were waited; -15 is normal namespace teardown. Complete sanitized daemon JSONL is retained. Gate and independent reviewer cleanup are recorded separately. Sidecars are deduplicated after the run, never as a runtime abort condition.

Independent **claude-opus-4-6** evidence review **approved**, exit **0**, one request **$0.14575000**. Two minor clarity notes, no required fix: raw/final classification distinction and explicitly incomplete total spend. Known current seats plus review **$0.15883100**, plus unknown foreign startup-notify usage; implementer/orchestrator spend unavailable. Reviewer child exited, scratch root removed, credential placeholder zero bytes. No new runtime or controller fix followed the stop.

Final archive audit: **390 snapshot paths → 104 unique objects**, all **211 complete daemon JSONL rows** retained, zero excluded usage rows; 21 pane captures checked at ≤60 lines. No forbidden operator path or secret pattern found. Independent process audit found **zero survivors** across all four scratch namespaces; all roots removed. Run6 sidecars unchanged. Packed paths resolve through `snapshots.json` → `objects[aliases[path]].content`; full daemon JSONL stays directly readable.
