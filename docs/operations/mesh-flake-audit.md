# Mesh E2E flake audit

Baseline: `0a22cf7c`, branch `fix/mesh-e2e-flake-audit`. Implementer: GPT-6
Astra. Scope: test helpers/specs; no product changes. This is a lane report,
not a plan ledger. P1 = lost click across runtime polling; P2 = scanner-state
propagation. “Fixed” refers to deliverable 2, commit `275d42d6`.
Sections 2–4 preserve the round-1 record; section 5 corrects its findings and
records round-2 verification.

## 1. Sweep

The audit table and cadence evidence now live in the
[testing guide](./testing-guide.md#mesh-runtime-flake-audit).

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

## Known residual (orchestrator ruling, 2026-09-06)

The earlier ruling described roughly **2/10 full-suite runs** as propagation
failures and delegated re-evaluation to the test-strategy lane. That product-vs-
timing conclusion is superseded by the orchestrator's 2026-09-06 evidence:
**3/3 failures** — reform branch full suite, reform branch standalone, and main
standalone. Two failures are standalone, so suite contention does not explain
the observation. This is not a measured 30% rate or a new timeout percentile.

Cold-resume's tail waits for zero offline members after Resume Team, the same
team-daemon startup-verification path cited by the two permanently skipped
recovery siblings. The test-strategy round therefore declares this one test a
known-issue exclusion, with this docket and the resume-verification product
issue named in run accounting. It leaves every stop/reload/resume assertion
intact for the product lane. No timeout growth, retry, or weakened assertion
is a fix for this issue. The other two existing product-issue skips receive
the same explicit accounting reason; the inverse-availability case remains
conditional. A green tier-1 now excludes these named cases visibly and does
not claim resume verification works.
