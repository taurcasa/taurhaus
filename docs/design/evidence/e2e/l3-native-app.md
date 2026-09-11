# UNAVAILABLE — run2 native setup failed (harness)

Run2 on `1801c6ea1d846103aa696f2d7231f468b310d5b8` (product base `64df9ffd4`) passed the syntax check and dry module load, then ran `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-hosted-builder` once, unchanged since import fix `0c042a162`. The real Linux app booted, but the spec's setup hook stopped with `WebDriverError: [object Object] when running "execute/async" with method "POST"`; the native command exited **1**. WDIO registered all six cases and executed none. The captured observer has empty IPC/gate arrays, and the screenshot shows the project Overview. Classification: **harness**, with the underlying cause unadjudicated; no product defect is established. No retry or fix was attempted; the orchestrator adjudicates this observation. [Native log](l3-native-app/run2/native.log), [WDIO receipt](l3-native-app/run2/run-summary.json), [screenshot](l3-native-app/run2/step-1.png), [IPC capture](l3-native-app/run2/step-1-ipc.json).

| Step 1 — boot and capability gate | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Digest, private protocol, installed/bundled contract, disabled Initialize then Canonical | NOT RUN: setup FAIL | harness | App booted; startup reports protocol 27 on private port 20144. No numbered-case RPC or capability assertions ran. [Preflight/digests](l3-native-app/run2/preflight.json), [daemon JSONL](l3-native-app/run2/daemon.jsonl). |

| Step 2 — builder and creation-time Delivery | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| alpha=tmux, beta=app_server, one Initialize, format 2, team ownership | NOT RUN | harness-blocked | No warmup or team initialization; no initialize payload or run result. |

| Step 3 — hosted startup and alpha exclusion | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Beta startup/thread versus daemon and pane; alpha has no hosted controls | NOT RUN | harness-blocked | No seats, hosted thread or pane receipts. |

| Step 4 — panel marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| One native input/reply, same thread/pane, acceptance before draft clear | NOT RUN | harness-blocked | No panel submission. |

| Step 5 — interrupt and fresh marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Active Stop turn, interruption, responsive controls, subsequent reply | NOT RUN | harness-blocked | No ordinary turn, interrupt or approval interaction. |

| Step 6 — reopen and host activity | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Same transcript/member, no new onboarding/host, host working→idle | NOT RUN | harness-blocked | No team to reopen; no journal or recovery receipts exist. |

Every spend: **0 warmup starts, 0 seat starts, 0 onboarding generations, 0 paid inputs/turns, USD 0**. Requested seats were Codex 0.153.4, `gpt-5.6-luna` / `low`; no model or effort actually ran. WDIO wall time was **6.375 seconds excluding its 20.827-second build**; outer elapsed time including build and cleanup was **28.579 seconds**. [Cost ledger](l3-native-app/run2/cost-ledger.json), [outer receipt](l3-native-app/run2/outer-cleanup.json).

[Mesh preflight](l3-native-app/run2/preflight.json) exited **0**: version **0.3.0**, protocol **1**, schema **1**, commit **9963e2030ced3b4d86bdd904afb9c3ff3cee7171**. `just build-daemon` exited **0** using this checkout's target. The recorded debug app/daemon digests identify the native binaries. The disabled/enabled capability transitions and app-reported installed/bundled contracts remain unverified.

[Cleanup audit](l3-native-app/run2/cleanup-audit.json): private PID namespace reaped; no surviving lane processes or private listener; worker root and both credential copies removed. Only the explicitly authorized `/home/mstie/.codex-account-b/auth.json` was copied by `createCodexScratchHome`; operator harness homes were hidden from children. Port 17233 and the operator tmux server were not contacted. [Launch receipt](l3-native-app/run2/attempt.json), [spec cleanup](l3-native-app/run2/cleanup.json).

Post-teardown gates: **`just check-quick` 0** (150 frontend files / 2,522 tests), **`just lint` 0**, **`just test-contracts` 0**. [Gate receipts, Cargo admission and final log lines](l3-native-app/run2/gates.json). No `src-tauri/` diff; the additional Rust unit gate does not apply.

Deviations/limits: no numbered case reached green, so there are no numbered-step commits. No new tests or source changes were made; the existing regression guard names `f2b5eab16`. No Opus evidence lens ran; adjudication remains with the orchestrator. Screenshots for steps 2–6, initialize/hosted IPC, journal export and receipts are unavailable because setup failed. The unmodified spec's evidence directory was redirected through a private bind mount and copied to `run2/` at teardown, preserving run1 sidecars; the WDIO dirty flag includes that evidence overlay. Run1 remains recorded in `1801c6ea1` and the original sidecars.
