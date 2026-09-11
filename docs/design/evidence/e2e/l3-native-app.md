# UNAVAILABLE — run3 step 1 stopped on a harness predicate

The final native attempt reached step 1 and stopped with `harness: capability resolution was not observed` (recipe exit **1**). Its observer recorded **no IPC calls**, while the DOM samples showed Initialize disabled and Canonical changing from unchecked to checked; the final screenshot shows Canonical checked and an empty roster. The real app booted and its private daemon answered protocol 27. These observations do not establish a product defect or a passing capability gate. No numbered-step retry followed; the orchestrator adjudicates this harness stop. [Screenshot](l3-native-app/run3/step-1.png), [IPC/DOM capture](l3-native-app/run3/step-1-ipc.json), [native log](l3-native-app/run3/native.log), [boot receipt](l3-native-app/run3/boot.json).

| Step 1 — boot and capability gate | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Executable digest, private protocol, installed/bundled contracts, Initialize gating | FAIL | harness | App digest and private protocol 27 recorded; lock-matching Mesh 0.3.0 observed. App-reported contract payload and timed capability assertion unverified. [Boot](l3-native-app/run3/boot.json), [RPC](l3-native-app/run3/daemon-rpc.json). |

| Step 2 — builder Delivery selections | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| alpha=tmux, beta=app_server, Initialize once, format 2, team owner | NOT RUN | harness-blocked | No warmup or initialization; no creation payload/run result. |

| Step 3 — hosted startup and alpha exclusion | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Beta transcript/thread versus daemon and pane; alpha has no hosted controls | NOT RUN | harness-blocked | No seats or hosted thread. |

| Step 4 — panel marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| One native input/reply, stable thread/pane, draft clears after acceptance | NOT RUN | harness-blocked | No panel submission. |

| Step 5 — active interrupt and fresh marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Stop turn once, observed interruption, responsive controls and subsequent reply | NOT RUN | harness-blocked | No ordinary turn, interrupt or approval interaction. |

| Step 6 — reopen and host activity | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Same transcript/member, no new onboarding/host, host working→idle | NOT RUN | harness-blocked | No team to reopen; journal and recovery receipts do not exist. |

The pre-seat regression check first reproduced the original `execute/async` hook error. After copying `invokeTauri` and `invokeTauriOrThrow` verbatim from `managed-stage-codex.js`, the read-only command returned 24 roles with zero seat starts; setup then rejected an empty behavioral contract. The second permitted hook retry retained a brief contract bullet and completed setup, reaching step 1. Both regressions originated in `4af0fc629a` and carry comments in the spec. The reference helpers actually use `window.__TAURI_INTERNALS__`, a single payload argument, an invoke guard and an `{ok,result|error}` envelope; they were preserved verbatim. [Original red](l3-native-app/run3/hook-red-native.log), [role-validation red](l3-native-app/run3/hook-retry1-native.log), [successful read-only proof](l3-native-app/run3/ipc-preflight.json), [WDIO receipts for all three boots](l3-native-app/run3/run-summary.json).

Every spend, across the original check and both hook retries: **0 warmup starts, 0 seat starts, 0 onboarding generations, 0 paid inputs/turns, USD 0**. All three ledgers contain empty reservations and turns: [original](l3-native-app/run3/hook-red-cost-ledger.json), [retry 1](l3-native-app/run3/hook-retry1-cost-ledger.json), [final](l3-native-app/run3/cost-ledger.json). Requested seats were Codex 0.153.4, `gpt-5.6-luna` / `low`; the CLI version was verified, but no model ran. Total WDIO runtime excluding builds was **21.426 seconds** across three boots; outer elapsed time including builds and cleanup was **102.288 seconds**. All native recipe exits were **1**. No paid retry occurred.

[Preflight](l3-native-app/run3/preflight.json): `just build-daemon` exited **0**, using this checkout's target. `just mesh-verify-lock` exited **0** against an executable copy of the bundled binary: version **0.3.0**, protocol **1**, schema **1**, commit **9963e2030ced3b4d86bdd904afb9c3ff3cee7171**. Native launch used the same bundled bytes. Initial lock verification also passed, but its non-executable resource path fell through the resolver; the explicit-copy verification removes that ambiguity. App, debug/release daemon and Mesh digests are recorded. Product base is `64df9ffd4`; launch revision was `0bb214a59`, with only the recorded spec/evidence edits. No descriptor or product changes were made.

[Cleanup audit](l3-native-app/run3/cleanup-audit.json): all three private PID namespaces reaped; no surviving lane process or listener on ports 23084, 20315 or 22885; all worker/staging roots and credential copies removed. Only the authorized `/home/mstie/.codex-account-b/auth.json` was copied with `createCodexScratchHome`; children saw hidden operator harness homes and a read-only staged source. Port 17233 and the operator tmux server were not contacted. [Final cleanup](l3-native-app/run3/cleanup.json), [outer receipt](l3-native-app/run3/outer-cleanup.json).

Post-teardown gates: **`just check-quick` 0** (150 frontend files / 2,522 tests), **`just lint` 0**, **`just test-contracts` 0**. [Gate exits, Cargo admission and last log lines](l3-native-app/run3/gates.json). No `src-tauri/` diff, so the additional Rust unit gate does not apply.

Deviations/limits: no numbered step passed, so no numbered-step PASS commits exist; the proven setup fixes have their own commit. Two pre-seat hook retries exhausted the allowed retry count. No independent Opus evidence lens was available; review remains with the orchestrator. Screenshots for steps 2–6, initialize/hosted IPC, journal export and receipts are unavailable because execution stopped before team creation. The sidecar named [daemon.jsonl](l3-native-app/run3/daemon.jsonl) contains the app's complete structured rows, including daemon bootstrap/RPC events; a standalone daemon log was not captured. Existing paid registration and documentation were retained; no offline controller suite or product fix was added. Prior run evidence remains in its original sidecars and history.
