# Mesh E2E flake audit

Baseline: `0a22cf7c`, branch `fix/mesh-e2e-flake-audit`. Implementer: GPT-6
Astra. Scope: test helpers/specs; no product changes. This is a lane report,
not a plan ledger. P1 = lost click across runtime polling; P2 = scanner-state
propagation. “Fixed” refers to deliverable 2, commit `275d42d6`.
Sections 1–4 preserve the round-1 record; section 5 corrects its findings and
records round-2 verification.

## 1. Sweep

All seven specs containing Mesh UI selectors were read, including their local
helpers. Rows group repeated calls to the same helper; distinct transitions
are listed separately. The four paid specs drive coordination through IPC,
not Mesh runtime UI, and are excluded from the suite and this change.

| Site / action → wait | Pattern | Disposition and reason |
| --- | --- | --- |
| All seven specs: `openMeshTab` → surface / gate resolution | — | Not applicable: titlebar is outside the polled runtime subtree; gate resolves availability IPC. |
| `mesh-workflow`: overview → Mesh tab switching | — | Not applicable: shell navigation, not scanner state. |
| `meshBuilder.setInlineBuilderTeamName` → input, then setup | — | Already safe for these patterns: setup is not runtime-polled; native input event drives the transition. |
| `mesh-workflow`: lead selection → lead card | — | Not applicable: local setup roster. |
| `mesh-workflow`: initialize enable / click → runtime or error | — | Not applicable: command completion, not scanner propagation; never retry a launch. |
| `mesh-workflow`: primary action → add-agent form | P1 | Fixed: single runtime click can vanish. |
| `mesh-workflow`: role/name/project → enabled submit | — | Already safe: editor state; role card is clicked atomically in the active SlideOver. |
| `mesh-workflow`: submit → form closed or error | — | Already safe: command result checked explicitly; never retry a mutation. |
| `mesh-workflow`: overflow → disband action | P1 | Fixed: target-first retry avoids losing or re-toggling the menu. |
| `mesh-workflow`: disband action → open confirmation | P1 | Fixed: retry the opener, not the confirmation. |
| `mesh-workflow`: confirm → empty/setup; reset → empty | — | Not applicable: disband command result and local reset. |
| `mesh-recovery`: overflow → menu; disband → confirmation | P1 | Fixed: both runtime openers are single clicks. |
| `mesh-recovery`: confirm → empty/setup | — | Not applicable: command completion; ownership checked before disband. |
| `mesh-recovery`: builder lead/agent → cards; input → setup | — | Not applicable: local setup state, no live runtime poll. |
| `mesh-recovery`: initialize → runtime/error/title | — | Not applicable: command completion and title identity, not member liveness. |
| `mesh-recovery`: kill all panes → offline count → coldResume snapshot | P2 | Fixed: 25s overrides undercut honest scanner propagation budgets. |
| `mesh-recovery`: reload → app/projects → stopped runtime copy | P2 | Fixed for runtime copy; app/project boot retains its own readiness checks. |
| `mesh-recovery`: resume click / IPC → zero offline → active runtime copy | P2 | Fixed: one launch followed by scanner/UI propagation, not repeated resume clicks. |
| `mesh-recovery`: kill member → offline count / degraded snapshot / UI copy | P2 | Fixed; existing product-issue skip remains visible. |
| `mesh-recovery`: named agent node → matching detail | P1 | Fixed: current loop stops at click delivery, not matching detail. |
| `mesh-recovery`: detail → Offline status | P2 | Fixed: status is fed by runtime polling. |
| `mesh-recovery`: Add Agent (primary or secondary) → form | P1 | Fixed: retry only the open, after active-state validation. |
| `mesh-recovery`: role/name/project → submit → error/form/message | — | Not applicable: command/editor state; final named-node assertion still required. |
| `mesh-recovery`: successful add → named node | P2 | Fixed: roster visibility follows runtime refresh. |
| `template-crud-ui`: runtime overflow → disband; disband → open dialog | P1 | Fixed: last-element single clicks still race re-renders. |
| `template-crud-ui`: confirm → empty/setup; reset → empty | — | Not applicable: command completion and local reset. |
| `template-crud-ui`: lead → card; initialize → runtime/error/title | — | Not applicable: setup and initialize response, not scanner propagation. |
| `template-crud-ui`: Add Agent → form, including rebuild branch | P1 | Fixed: both previously safe local loops now use the same shared helper. |
| `template-crud-ui`: runtime node → detail capture button | P1 | Fixed: cached node handle can be replaced before click. |
| `template-crud-ui`: detail capture → capture form | P1 | Fixed: target-first retry for the runtime detail opener. |
| `template-crud-ui`: role card → autofill; unlock → enabled; cancel → closed | — | Not applicable: stable SlideOver editor state, not runtime node replacement. |
| `template-crud-ui`: capture save → success banner → catalog card | — | Already safe: atomic active-SlideOver save and exact success text; never retry save. |
| `template-crud-ui`: template browser → panel | — | Not applicable: empty/setup view is not runtime-polled. |
| `template-crud-ui`: create/edit role → editor; inspect → detail | — | Already safe: active-SlideOver query and DOM click share one browser task. |
| `template-crud-ui`: role save → persisted role / instructions | — | Not applicable: git-backed command result, not daemon scanner. |
| `template-crud-ui`: role delete → dialog → card absent | — | Already safe for P1/P2: atomic opener; final deletion assertion remains. |
| `template-crud-ui`: presets tab → create; create → customizer | — | Already safe: atomic SlideOver clicks, no runtime poll. |
| `template-crud-ui`: preset save → card / customizer closed; delete → dialog → absent | — | Not applicable: storage command and catalog refresh. |
| `template-crud-ui`: cleanup close/cancel → SlideOver closed | — | Already safe: bounded existing atomic clicks with closed-state checks. |
| `templates`: close overlay → absent; reset → empty; browser → panel | — | Not applicable: setup/catalog only; refuses every runtime disband. |
| `templates`: roles/presets tabs → cards; inspect → exact detail text | — | Already safe: atomic active-SlideOver clicks. |
| `templates`: upsert/flush → reopen → card/details; delete/flush → reopen → absent | — | Not applicable: awaited storage IPC, no scanner dependency. |
| `mesh-screenshots`: runtime disband opener → confirmation | P1 | Fixed: open overflow first and await an actually open dialog. |
| `mesh-screenshots`: confirm → empty/setup | — | Not applicable: disband command completion. |
| `mesh-screenshots`: display → setup; customize → panel → input | — | Not applicable to timing classes: legacy setup selectors/input flow; report drift if executed. |
| `mesh-screenshots`: customizer save → closed; initialize → runtime/failure | — | Not applicable: command completion, never repeat mutations. |
| `mesh-screenshots`: theme → capture | — | Not applicable: shell styling, no propagation wait. |
| `template-screenshots`: runtime cleanup disband → confirmation | P1 | Fixed: same runtime opener shape, behind existing ownership guard. |
| `template-screenshots`: init-back/reset → setup/empty; wait initializing exit | — | Not applicable: local setup or initialize response. |
| `template-screenshots`: preset/custom → setup; add roles → counts; pin → strip | — | Not applicable: setup-only local roster; legacy selectors are separate drift. |
| `template-screenshots`: overlay close → absent; theme → screenshot settle | — | Not applicable: setup overlays and visual capture. |
| `role-detail-screenshots`: setup/search → catalog; role info → detail | — | Not applicable: catalog preview, not a live runtime node. |
| `role-detail-screenshots`: edit → inputs; cancel/close → absent | — | Already safe for P1: atomic overlay clicks; existing bounded edit retries are setup-only. |
| `role-detail-screenshots`: create → editor; save → closed/card/searchable | — | Not applicable: local editor/storage response (existing git-write budget retained). |

## Cadence and safety evidence

`daemon/session_activity.rs` uses 500ms active / 1500ms idle scan intervals;
`meshTabGate.svelte.js` uses 2000ms live-status polling. These are separate
from the 30s background self-heal pass; recovery queries reconcile live
presence through the daemon directly (`commands/coordination/live_status.rs`).
Propagation budget: four idle scans (6000ms), two UI polls (4000ms),
plus 20000ms scheduling/IPC margin for suite contention = 30000ms. This is a
bounded allowance, not a promise that product work has a 30s upper bound.

The manifest currently runs one worker at a time (`maxInstances: 1`), despite
the historical seven-worker incident. Acceptance runs must use the requested
unmodified command/configuration, not introduce a load run. Paid lanes stay
excluded. Worker roots/tmux/daemon are isolated; any default harness commands
must be shadowed by inert fixtures before the suite is started.

The two unconditional recovery skips and prerequisite skips predate this
lane. No new skip is authorized. A failed wait remains a failure. Failure
screenshots can show cleanup's disband, so diagnosis must use the preceding
app log, as documented by `673dac42`.

## 2. Implementation

`clickUntil.js` owns the single target-first retry loop. It accepts test IDs
or fresh-query callbacks for named nodes and open dialogs. Five specs use it;
both existing template Add Agent branches were retrofitted. Initialization,
resume, submit, save and final confirmation remain single actions.

Tests were written first. After the initial missing-export red, the extracted
old single-click / 25s behavior was exercised: **5 failed, 2 passed**. Lost
click and named-detail cases timed out, an already-open target was clicked,
the never-open case had only one attempt, and healthy state at 28s timed out.
The target-first implementation / 30s budget passed **7/7**. Tests use virtual
time and mocked browser globals; they start no process or CLI.

## 3. Assertion and skip integrity

Every existing `expect(` and `.skip(` call line in the five edited specs was
compared against `0a22cf7c`: all unchanged. The predicates for exact offline
count, snapshot team/state, runtime copy/label/summary, named-node presence,
role autofill/disabled fields and persistence remain intact. No wait turns
failure into success. Disband now requires an open dialog; the named-node
opener no longer converts a timeout to `false` before its assertion.

The helper's never-open test verifies failure with the original caller
message. Its already-open and between-poll cases verify no extra click, and
the wrong-member test verifies that another member's detail is insufficient.
The propagation test also rejects state arriving after the 30s bound.

`just check-quick`: exit 0; 142 Vitest files / 2416 tests passed. No Rust diff,
so the additional Rust-unit execution rule is not triggered. No plan ledger
was edited. Gates and E2E proof are recorded below when completed.

## 4. Round-1 acceptance proof (superseded)

The record below describes the original implementation runs. Round-2 review
subsequently observed **155 passing / 26 skipped** in its third run versus
**157 / 24** in the first two: both mesh-workflow tier-2 cases were skipped.
Thus round 1 did not establish skip-set stability; see the round-2 record below.

All commands ran from this checkout root, without piping gates. Each exit
code was captured immediately with `rc=$?`. No test-source changes occurred
between the three E2E runs.

| Gate | Outcome |
| --- | --- |
| `just check-quick` | Exit 0; Rust fmt/check, frontend typecheck, 142 files / 2416 tests passed. |
| `just lint` | Exit 0; Clippy, frontend structure/dependency checks, workflow syntax and gate guards passed. |
| `bunx vitest run` | Exit 0; 142 files / 2416 tests passed. |
| `E2E_INSTALL_DAEMON=0 just test-e2e` — run 1 | Exit 0; `Spec Files: 9 passed, 9 total (100% completed) in 00:04:38`. |
| `E2E_INSTALL_DAEMON=0 just test-e2e` — run 2 | Exit 0; `Spec Files: 9 passed, 9 total (100% completed) in 00:04:39`. |
| `E2E_INSTALL_DAEMON=0 just test-e2e` — run 3 | Exit 0; `Spec Files: 9 passed, 9 total (100% completed) in 00:04:35`. |

Each E2E run reported **157 passing / 24 skipped**; WDIO's nine spec-file
entries are the sealed worker groups. All three explicitly passed cold
stop/reload/resume, initialize/hot-add/disband, role-aware Add Agent autofill
and unlock, and runtime-node role capture. The template group passed all 14
cases; the Mesh group passed five and retained six skips (the two documented
product-issue cases, three inverse-prerequisite cases, and the screenshot
ownership guard). These are limitations, not new skips. The artifact hook
also captures some pending cases, so an artifact directory alone is not a
failed test; the final reporter and exit code establish the outcome.

Safety setup for all three runs: inert Node harness executables named
`claude`, `codex`, `agy`, `grok`, and `gemini` were generated in the ignored
`.check-logs/flake-audit/cli-bin` directory and prepended to PATH. They exit on
SIGINT/SIGTERM, print only a fixture version for `--version`, and otherwise
idle; they never invoke an AI CLI. Existing spec-specific harness fixtures
remain in use. `E2E_WDIO_PORT=42367`, `E2E_NATIVE_WEBDRIVER_PORT=42368`, and
per-run `E2E_WDIO_OUTPUT_DIR` only select private driver ports/artifact paths.
No worker-count, retry, bail, Mocha-timeout or build-skip override was used.
The existing worker hooks own cleanup. Final verification found **zero owned
E2E processes**, all **27/27 worker roots removed**, zero remaining checkout
process ledgers, and both driver listener ports released. Generated docs
screenshots were removed; the original untracked `LANE-SPEC.md` was preserved.

Local evidence is retained under `.check-logs/flake-audit/`: `check-quick.log`,
`lint.log`, `vitest.log`, `item-2-behavior-red.log`, `item-2-green.log`, and
`e2e-run-{1,2,3}.log` with matching `.rc` files. No product bug blocked this
lane and no product source was changed. The default configuration is serial,
so these runs do **not** prove seven-worker behavior.
Review/routing calibration belongs to the independent reviewer; the
implementer does not assign its own quality score. Review-relevant choices:
one 20-line retry helper, no product edits, deterministic fault injection,
and no new skip or repeated mutation.

## 5. Round-2 corrections

`mesh-workflow` now requires successful app and mesh/tmux readiness in its
`before` hook. RPC failure, incomplete availability, a blocking setup surface,
unsafe runtime cleanup, and initialization errors fail coverage instead of
turning its two required tier-2 cases into skips. The two inverse-environment
cases explain their stable skip fact: the worker has mesh and tmux installed.
This is a hard precondition, not a scanner-cadence guess for a binary lookup.

`meshRuntime.js` checks the primary action's enabled Add Agent label and clicks
in one browser task, on every retry. A Resume control is never clicked. All
five confirmation sites use `confirmDialog.js`, which scopes to the open dialog
and retains `fastClick`'s intercepted-click fallback. Named-node selection
asserts the detail heading at the caller instead of an always-true return.

The UI cadence is imported from `meshTabGate.svelte.js`; the Rust idle cadence
is named locally. The 20s margin covers worker-load variance in probes and IPC.
Its basis is the historical failure of 20/25s budgets, not a measured percentile;
the 28s regression boundary is virtual. No load or stress measurement was run.

Red-first tests execute the real workflow hooks/cases with fake IPC/DOM:
9 failed / 1 passed before the precondition fix, then 10/10 passed. The extracted
unsafe opener/confirmation behavior produced 6 failures / 1 pass, then 7/7
passed. WDIO Timer error-shape tests produced 2 failures before correcting the
fake, then 7/7 helper tests passed. No test starts a real harness.

The second preliminary E2E run failed at the existing single-click unlock in
`template-crud-ui`: `Role-aware unlock did not re-enable editable fields`.
The preceding logs show initialization completed and runtime snapshots continued;
disband was the test's later cleanup, not the cause. The original sweep's
"not applicable" classification for unlock was too broad. A lost-click test
and an already-unlocked test failed against the extracted single-click flow
(2 failed / 4 passed). Unlock now uses the same target-first helper without
changing the editable-field assertion or its timeout. Final proof restarts
after this correction; the failed run is retained in the preliminary logs.

The next sequence's second run exposed the original skip race as a hard
failure: recovery had disbanded its team and cleared shared ownership, but the
following spec still rendered the deleted team's cached runtime. The diagnostic
was `Mesh setup precondition failed: Refusing to disband runtime team outside
the sealed e2e group`. The workflow now refreshes once in `before`, then runs
normal app readiness and availability checks against a fresh project snapshot.
The ownership guard remains unchanged. A real-spec regression reproducing the
stale view failed before this change (1 failed / 10 passed), then passed.

A later full run passed Mesh but failed the untouched session-management hover
assertion: expected `Active work in progress`, received an empty string. Its
failure screenshot already shows the expected text. That separate transient
read was retained under `preliminary-hover/`; no session source was changed.
Another sequence exposed a single-attempt role-inspect opener after edit
persistence, before the editor had yielded to the catalog. The actual edit
case failed under injected delayed-control readiness (2 red tests); it now
uses `clickUntil` for inspection only, preserving the save count, detail
assertion and 6s deadline. Both regression tests pass. These failed sequences
do not count toward the required three consecutive full-suite greens.
