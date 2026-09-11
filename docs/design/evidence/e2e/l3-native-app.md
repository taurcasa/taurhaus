# Linux native app lane 3

**UNAVAILABLE — harness failure before step 1; no retry.** The single `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-hosted-builder` attempt at commit `6581a8e8765e13855b6ec1f522cbb55040a6e448` built and booted the real Linux app, but WDIO could not load the spec: `ReferenceError: $state is not defined`. The policy import added in `f2b5eab16` reaches `toolRegistryState.svelte.js` through `meshTabUtils.js`; Node has no compiled Svelte runes. No spec hook or numbered case executed. The executed spec is retained at that commit for orchestrator adjudication; the later harness-only fix `0c042a162` reads the authoritative JSON policy without importing Svelte and corrects the scratch data-directory spelling. It has not been run natively. [Raw log](l3-native-app/native.log), [WDIO receipt](l3-native-app/run-summary.json). Startup stderr also records `state() called before manage() for taurhaus_lib::WatcherState`; that separate Taurhaus observation is not established as the cause of this harness failure.

| Step 1 — boot and capability gate | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Executable digest, private protocol, installed/bundled contract, disabled Initialize then Canonical | NOT RUN: setup FAIL | harness | The app booted, but no capability assertion or private protocol RPC ran. [Binary digest and boot count](l3-native-app/run-summary.json), [Mesh preflight](l3-native-app/preflight.json). |

| Step 2 — builder and creation-time Delivery | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| alpha=tmux, beta=app_server, one Initialize, format 2 and team ownership | NOT RUN | harness-blocked | Module loading stopped execution before warmup, roster selection or initialization. No initialize IPC payload, daemon run result or team exists. |

| Step 3 — hosted startup and alpha exclusion | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Beta's rendered startup/thread versus daemon and pane; alpha has no hosted controls | NOT RUN | harness-blocked | No seats launched; no transcript, thread or pane receipt. |

| Step 4 — one panel marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| One native input/reply in the same thread and pane; draft clears after acceptance | NOT RUN | harness-blocked | No panel input submitted. |

| Step 5 — interrupt and fresh marker | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| One active Stop turn, observed interruption, responsive controls and fresh reply | NOT RUN | harness-blocked | No ordinary turn, interrupt or approval interaction occurred. |

| Step 6 — reopen and host activity | Outcome | Classification | Evidence |
| --- | --- | --- | --- |
| Same member/transcript, no onboarding or host relaunch, host working→idle | NOT RUN | harness-blocked | No member existed to reopen; no activity or journal receipts. |

Every spend: **0 paid inputs, 0 seat starts, USD 0**. The worker ran for approximately 5.410 seconds excluding its 110.744-second app build. [Cost ledger](l3-native-app/cost-ledger.json). Mesh lock verification exited 0: version **0.3.0**, protocol **1**, schema **1**, commit **9963e2030ced3b4d86bdd904afb9c3ff3cee7171**. Codex **0.153.4** was observed by the app's version probe; no model or effort actually ran. The checkout's protocol-27 daemon was built, but its protocol was not independently read over RPC in this failed attempt.

[Launch command and pinned binary digests](l3-native-app/attempt.json), [namespace teardown](l3-native-app/outer-cleanup.json), [process/root/auth cleanup receipt](l3-native-app/cleanup-audit.json). The private daemon port was **23257**; port 17233 and the operator tmux server were not contacted. Operator harness homes were hidden in a private user/mount/PID namespace; only the explicitly authorized `auth.json` was staged with the existing scratch-home helper. Both credential copies and the worker root were removed; no surviving process carried the lane environment. The WDIO receipt's dirty flag reflects this new untracked evidence directory.

Screenshots, frontend IPC captures, daemon JSONL and journal exports are **unavailable**: the spec failed during module loading before its capture hooks were registered, and normal worker cleanup removed the private roots. No substitute artifacts or passing outcomes are fabricated. No Opus evidence review ran in this implementer session. The only source-scope additions beyond the spec, manifest and one paid-table line are updates to two existing inventory assertions required by the new spec. Six numbered-case commits were followed by two pre-attempt preparation commits and one post-attempt harness fix; native red→green was not obtained because the one-attempt stop rule applies. [Post-teardown gate receipts](l3-native-app/gates.json).
