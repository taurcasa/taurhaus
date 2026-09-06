# Testing guide

Testing strategy, test lanes, and procedures for the taurhaus project.

## Overview

Testing follows TDD for logic and visual review for layout. The maintained lanes are Rust tests, frontend Vitest tests, browser-mode visual tests, and E2E tests via WebdriverIO. The per-task verification gate is `just check-quick`.

## Philosophy

- **TDD for logic** — red, green, refactor. Write the failing test first, make it pass, clean up.
- **Visual review for layout** — UI appearance is verified visually, not through pixel-perfect assertions.
- **AC-driven coverage** — every acceptance criterion gets a test. No numeric coverage targets.
- **Regression guards** — every bug fix ships with a test that stays forever. Non-negotiable.

## Lane and PR acceptance

Product changes require one `E2E_INSTALL_DAEMON=0 just test-e2e-smoke` pass
beside the static/unit gates; documentation-only changes are exempt. This
one-session native smoke saves settings via the real frontend payload and
reloads persisted state, closes SlideOver before keyboard navigation, reads a
startup project, and reconnects the private worker daemon before useful work.
It never launches a model CLI. Full behavioral breadth is risk-triggered and
required at milestones; one complete pass is sufficient, not three automatic
repeats. Reuse `E2E_SKIP_BUILD=1` only with an already verified fresh build.
Ordinary workers put rejecting `claude`/`codex`/`agy`/`grok` shims first on
their isolated shell PATH. Runtime tests must supply their own generated CLI
stubs; missing stubs cannot fall through to an installed real harness. Only
the explicitly named paid workers omit those shims.

Only `first-run-wizard.js` gets a virgin worker root. Other workers scan and
batch-register generated fixture repositories through the wizard's supported
Tauri commands, then reload the frontend. Both paths assert exactly `ledger`
and `taurhaus` are registered and `is_first_run` is false. A missing seed must
fail; it must never fall back to another UI wizard walk. Settings saving remains
a real UI action in the smoke, independent of onboarding setup.

Each WDIO invocation prints a unique `run-summary.json` path under its log
directory. It records selected/executed/passed/failed/skipped/unreached counts
per spec, skipped test names, revision (with dirty-tree flag), and binary SHA-256.
Files selected but never loaded remain null, rather than looking passed. The
completion hook fails even if WDIO exits zero when selected tests were skipped
or not reached. Missing prerequisites are failures of required coverage; paid
lanes and explicit file exclusions are declared separately. A fail-fast run is
useful diagnosis, not evidence of complete breadth. Both `test-e2e` and
`test-e2e-full` disable WDIO and Mocha bail. Named specs and smoke retain the
local fail-fast defaults; bare WDIO can opt into breadth with `E2E_BAIL=0
E2E_MOCHA_BAIL=0`.

`just capture-e2e-docs` produces general and README screenshots on demand;
these files are excluded from acceptance. The native light/dark transition
assertions from the old `screenshots.js` remain in its behavioral spec.
The default behavioral manifest retains nine serial sessions: eight seeded,
one virgin wizard. This removes eight repeated wizard walks while preserving
the existing session isolation. The former capture session is replaced by the
dedicated wizard session; the small smoke shares the UI group during breadth.
`maxInstances: 1` and private headless Xvfb remain unchanged. The summary also
records `boots`, `wizard_walks`, `wall_ms` (from launcher preparation, including
build), `build_ms`, and whether the build was reused. No parallelism change is
justified by this lane. Before: nine boots and nine wizard walks per suite;
after: nine boots and one wizard walk. Historical whole-suite wall time was
not measured; compare future runs using the recorded build and execution costs.

## Test layers

### Rust tests

Per-module `#[test]` functions with `pretty_assertions` for readable diffs and `tempfile` for isolated filesystem tests.

```bash
just test-rust            # Full Rust lane (fast compile + unit + integration/system)
just test-rust-fast       # Compile check only (fast feedback)
just test-contracts       # Execute CLI goldens, module boundaries, harness conformance
just test-rust-unit       # Unit/bin tests, heavy suites excluded
just test-rust-integration # Every src-tauri/tests/*.rs binary plus the heavy --lib suites
just test-daemon-connectivity # Manual daemon chain verification (WSL/local)
```

Test placement follows two patterns:

Rust-touching lanes run `just test-contracts` beside `just check-quick` and
affected Rust tests. This promotes the launch-default and writer-boundary
defect catchers without replacing full Rust CI. Review changed golden defaults.
Measured on 2026-09-06: 64 contract tests passed; first incremental invocation
36.52 s (Cargo build 33.11 s), repeat 52.69 s including shared-target lock wait
and rebuild. Test-binary execution totals about 4.6 s; a contention-free warm
invocation and a cold build were not measured. These are not cold-build claims.
The lead `check` also executes `lint-just-gates` in its frontend lane; its
seeded integrity runs replace both lanes and therefore do not recurse.

- command-layer modules keep external sibling `tests.rs` files
- lower-level modules keep inline `#[cfg(test)] mod tests`

### Frontend unit tests

Vitest + JSDOM + `@testing-library/svelte`. Tests cover components, stores, and utility modules.

```bash
just test-frontend        # Run all frontend Vitest tests
just test-visual          # Run browser-mode visual screenshot tests
```

`just test-visual` resolves its own browser: `PLAYWRIGHT_CHROME_PATH` when set
(a path that does not exist fails the run and names itself), else
`/usr/bin/google-chrome`, else Playwright's managed Chromium. The run's first
line names the binary it launched. See
[`visual-testing-guide.md`](./visual-testing-guide.md#which-browser-the-lane-launches).

#### Full-window screenshots (`just visual-shot`)

`just test-visual` renders a component into a 960×640 test page. A popup that
positions itself against the *viewport* — the account chooser overlay, the
account chip's menu, the context menu and its submenus — cannot be judged
there: it needs a real window at a real size, with the app's own frame markup
around it.

```bash
just visual-shot shell-popups chooser-light laptop light        # prints the PNG's WSL path
just visual-shot shell-popups chip-menu-dark narrow dark shot   # custom output name
just visual-shot-stop                                           # stop the server it started
```

- Starts the visual host on port 5211 (`--strictPort`) **only if nothing is
  already listening there**, and `visual-shot-stop` kills only a pid it wrote
  down and re-verified with `ps`. Somebody else's `bun run dev:visual` is never
  touched.
- Shoots with Windows Edge headless (`msedge.exe --headless=new --screenshot`)
  against `http://localhost:5211/?component=…&scenario=…&viewport=…&theme=…&chrome=0`.
  `VISUAL_SHOT_EDGE`, `VISUAL_SHOT_PORT`, and `VISUAL_SHOT_WINDOWS_DIR` override
  the browser, port, and output directory.
- The URL is the fixture's address: the visual host reads `component`,
  `scenario`, `viewport`, and `theme` from `location.search`, and `chrome=0`
  drops its own controls so the shot is the fixture alone at window size.
- Viewports are the host's presets: `desktop` (1920×1080), `laptop` (1366×768),
  `narrow` (1024×768); themes are `light` and `dark` and nothing else (`exit 2`
  — the host falls back for a theme it does not know, so an accepted `drak`
  would file the scenario's own theme under that name).
- A shot is evidence, so every way of producing an irrelevant one fails instead:
  the listener on the port must identify itself as the visual host (`exit 6`),
  the page must report the state that was asked for — component, scenario,
  viewport and theme, written into `data-visual-host-fixture` and read back from
  the same Edge run's DOM dump (`exit 7`, the usual cause being a mistyped
  component or scenario; matched as a fixed string, so a name carrying `.` or
  `*` cannot match the host's fallback), the file must be a PNG whose IHDR says
  exactly the viewport preset's pixels (`exit 10` — the run forces
  `--force-device-scale-factor=1`, so a shot that comes back another size was
  rendered at another window size), Edge's exit status counts (`exit 8`), and
  the browser runs under a wall clock that insists: TERM, then KILL (`exit 9`,
  `VISUAL_SHOT_TIMEOUT_S` default 90 s, `VISUAL_SHOT_KILL_AFTER_S` default 5 s).
- PNGs land in `C:\taurhaus_build\shots` and are **not** committed — `*.png` is
  gitignored outside `docs/`. Paste them into the PR description as before/after
  evidence.

**Vitest cwd gotcha**: Vitest must run from the project root (`/home/user/projects/taurhaus`), not from `src-tauri/`. If `bunx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this automatically.

Test files follow the pattern `*.test.js` alongside the source they test (e.g., `src/lib/format.test.js`).

For manual visual review, run `bun run dev:visual` and use the fixture host documented in [`visual-testing-guide.md`](./visual-testing-guide.md).

### E2E tests

The [Mesh runtime flake audit](#mesh-runtime-flake-audit) below records opener
safety and scanner-derived wait budgets. The [lane report](./mesh-flake-audit.md)
retains acceptance runs, including skip sets.

WebdriverIO + `tauri-driver`. E2E tests launch the real app binary and interact with it through the accessibility tree. Linux only — Windows E2E is not supported due to shared app data directory conflicts.

```bash
just test-e2e             # Tier 1 — basic specs (no daemon required)
just test-e2e-full        # Tier 1 + Tier 2 (requires running daemon)
just test-e2e-spec SPEC   # Single spec file (e.g., just test-e2e-spec search-workflow)
just test-macos-e2e       # macOS E2E via SSH on remote Mac Mini
```

**Tiers**:
- **Tier 1**: Tests that work without a daemon connection (UI, navigation, settings)
- **Tier 2**: Tests requiring a running daemon (session detection, file watching, command center)

**E2E setup** (see [e2e/README.md](../../e2e/README.md) for troubleshooting):
1. Keep `E2E_INSTALL_DAEMON=0` (the default). Workers launch the checkout-local daemon; setting it to `1` only rebuilds and restarts the operator's installed daemon.
2. The recipes build the E2E binary automatically unless `E2E_SKIP_BUILD=1` is set
3. Run the tier/spec command you need

**Skip build** (when binary is known-fresh): `E2E_SKIP_BUILD=1 just test-e2e-spec SPEC`

Test specs live in `e2e/specs/` and are split by workflow/domain rather than by one monolithic suite.

Every worker isolates all five writable product roots beneath its session temp
directory: `TAURHAUS_DATA_DIR`, `TAURHAUS_CLAUDE_DIR`, `CODEX_HOME`, `GROK_HOME`
and the taurhaus-only Antigravity root `TAURHAUS_AGY_DIR`. The child `HOME` also
points beneath that directory, preventing implicit sibling-account discovery
under the operator home. Ordinary workers get an empty Codex home. Selecting a
paid lane is the only path that copies `auth.json` from the configured source
home into that scratch root.

Every worker also owns a non-default daemon port and a tmux server rooted at
`<session-temp-root>/tmux`; the runner clears inherited `TMUX` before the driver,
app, and daemons start. Teardown kills only that server. A unique inherited run
token identifies worker processes, which are persisted as PID plus `/proc`
start time in a checkout-scoped ledger. Pre-run cleanup considers only
abandoned ledgers from the same checkout and requires both fields to match, so
it cannot kill a concurrent run or a foreign process that reused a PID.

The WDIO manifest is sealed. `e2e/specList.js` explicitly assigns every
non-paid spec to a named group (`ui`, `templates`, `mesh`, and `tmux` name the
stateful additions); an ungrouped spec fails with instructions to add it to a
group, `paidSpecs`, or `captureSpecs`. The default suite is exactly the union of those groups,
and paid specs remain excluded.

#### Paid E2E lanes

Four specs drive a real Codex subscription and cost money every time they run. `e2e/specList.js` keeps all four out of the config's spec list, so no suite run — including a bare `bunx wdio run e2e/wdio.conf.js` — picks them up; each is started by name and nothing else starts it.

| Lane | Recipe | What it proves |
|---|---|---|
| `compaction-codex-hooks` | `E2E_INSTALL_DAEMON=0 just test-e2e-spec compaction-codex-hooks` | A managed Codex member gets its restored-context card back through the native hook bridge. See [compaction-testing.md](compaction-testing.md). |
| `managed-stage-codex` | `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex` | A managed Codex member completes a bounded task through the mesh assignment contract, with the assignment's effort put into force before the notice is delivered (W4 experiment 3). |
| `managed-stage-deadline` | `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-deadline` | Deadline semantics on a real managed member (W4 experiment 4): active work suppresses the half-time nudge and completes normally; an honestly started then silent one-minute task receives one nudge, becomes stale, yields the stage-shaped `timeout` verdict, and keeps its pane/session alive. Evidence comes from task, operational, activity, attention, inbox, runtime, and structured-event records. |
| `managed-stage-parallel` | `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-parallel` | Two medium-effort assignments run concurrently on two Codex members in two detached worktrees of one fixture repo (W4 experiment 5). Evidence is each checked-out HEAD tree's baseline diff, each member inbox's assignment IDs, distinct task `completion.at` values, and a positive-duration intersection of both attention-delivered-to-RESULT windows. Assigned-to-RESULT windows remain in the measured output as context. The W2 check is described below. |

All four use the same five isolated worker roots. Their scratch `CODEX_HOME` holds only a copy of `auth.json` plus a generated `config.toml`. The operator's `~/.codex` is read once at copy time and never written; `~/.claude`, `~/.grok` and `~/.gemini` are neither read nor written. Naming any paid lane on the command line is what tells `wdio.conf.js` to populate that already-isolated Codex home.

The three managed-stage lanes additionally set `CLAUDE_DIR` on the panes they create, because their members run `mesh` themselves: taurhaus passes `--claude-dir` to the member *daemon* it spawns but exports no Claude root into the pane, so without it a member's own mesh command would bootstrap the run's team inside the operator's real home. Their team lead is a Claude identity and an inbox, not a working agent — it is launched into the isolated, credential-free `CLAUDE_CONFIG_DIR` and never takes a turn, so these lanes spend nothing on Claude. Experiment 3's measured cost and wall clock are in [w4-experiment-3.md](../design/research/w4-experiment-3.md); the experiment 4 and 5 research notes are written from their named runs after each lane merges.

`managed-stage-parallel` is a one-spec measured lane, never a suite ingredient. It creates one fixture repository plus both member worktrees under the worker session temp root, launches exactly two Codex sessions, assigns both tasks through concurrent create+assign pipelines with `--effort medium --deadline 10`, and tears down the private tmux server in the shared interrupt-safe cleanup path. Run it once after merge for the experiment report.

The W2 evidence is a scanner-contract read-back over a production-shaped summary that the lane synthesizes after both live stages finish. Its task ids and RESULT timestamps come from those stages, and its label/phase vocabulary is parsed from `.claude/workflows/feature-pr.js`; the production scanner then has to surface both couriers from the one completed summary. The credential-free Claude lead never takes a turn and emits no Workflow summary, so this lane does **not** prove that a real lead run filed the stages. That is the documented experiment-5 item-(d) gap under the two-Codex-session cost ceiling. The synthesized courier phase is `Managed stage`, the production W2 phase for `stage()`, while the exec transport uses `Implement`; the phase name is not evidence that an implementation step went missing.

All paid lanes take on every host change they make as an undo (`e2e/helpers/laneCleanup.js`) that runs on interrupt as well as on teardown. Like every ordinary spec, they run on the worker's private tmux server; the tmux-driving source guard requires an isolation assertion before the first tmux call, and teardown takes the whole worker server down. All three managed-stage lanes additionally check the app's own `/proc/<pid>/environ` before they spend a turn.

## Mesh runtime flake audit

P1 = lost click across runtime polling; P2 = scanner-state propagation.
The [lane report](./mesh-flake-audit.md) retains implementation and acceptance
evidence. This table reflects the round-2 fixes.

All seven specs containing Mesh UI selectors were read, including their local
helpers. Rows group repeated calls to the same helper; distinct transitions
are listed separately. The four paid specs drive coordination through IPC,
not Mesh runtime UI, and are excluded from the suite and this change.

| Site / action → wait | Pattern | Disposition and reason |
| --- | --- | --- |
| All seven specs: `openMeshTab` → surface / gate resolution | — | Not applicable: titlebar is outside the polled runtime subtree; gate resolves availability IPC. |
| `mesh-workflow`: shared eligibility probe → tier-2 cases | — | Fixed: `before` refreshes the prior spec's cached view and requires app, mesh and tmux readiness; failures fail the hook instead of skipping both cases. |
| `mesh-workflow`: overview → Mesh tab switching | — | Not applicable: shell navigation, not scanner state. |
| `meshBuilder.setInlineBuilderTeamName` → input, then setup | — | Already safe for these patterns: setup is not runtime-polled; native input event drives the transition. |
| `mesh-workflow`: lead selection → lead card | — | Not applicable: local setup roster. |
| `mesh-workflow`: initialize enable / click → runtime or error | — | Not applicable: command completion, not scanner propagation; never retry a launch. |
| `mesh-workflow`: primary action → add-agent form | P1 | Fixed: fresh-query retry checks an enabled Add Agent label atomically; never clicks Resume. |
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
| `template-crud-ui`: role card → autofill; cancel → closed | — | Not applicable: local editor state. |
| `template-crud-ui`: unlock → editable fields | P1 | Fixed: full-suite evidence showed a lost click; target-first retry stops when fields are editable, without relocking them. |
| `template-crud-ui`: capture save → success banner → catalog card | — | Already safe: atomic active-SlideOver save and exact success text; never retry save. |
| `template-crud-ui`: template browser → panel | — | Not applicable: empty/setup view is not runtime-polled. |
| `template-crud-ui`: create/edit role → editor; inspect → detail | — | Already safe: active-SlideOver query and DOM click share one browser task. |
| `template-crud-ui`: role save → persisted role / instructions | — | Not applicable: git-backed command result, not daemon scanner. |
| `template-crud-ui`: saved edit → role inspection | P1 | Fixed: persistence can finish before the catalog control is ready; retry inspection, preserving the single save and final detail assertion (`4b4fb842`). |
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

### Cadence and safety evidence

`daemon/session_activity.rs` uses 500ms active / 1500ms idle scan intervals;
`meshTabGate.svelte.js` uses 2000ms live-status polling. These are separate
from the 30s background self-heal pass; recovery queries reconcile live
presence through the daemon directly (`commands/coordination/live_status.rs`).
Propagation budget: four idle scans (6000ms), two UI polls (4000ms),
plus 20000ms scheduling/IPC margin for suite contention = 30000ms.
The UI cadence is imported; the Rust idle cadence is named locally because it
is not JS-importable. The margin covers queued IPC and process-probe variance;
it is conservative headroom based on failed 20/25s waits, not a measured
latency percentile. The 28s unit-test boundary is injected virtual time. This is a
bounded allowance, not a promise that product work has a 30s upper bound.

The manifest currently runs one worker at a time (`maxInstances: 1`), despite
the historical seven-worker incident. Acceptance runs must use the requested
unmodified command/configuration, not introduce a load run. Paid lanes stay
excluded. Worker roots/tmux/daemon are isolated; any default harness commands
must be shadowed by inert fixtures before the suite is started.

The two unconditional recovery skips predate this lane and name product
issues. Workflow prerequisite failures now fail loudly; its inverse-environment
skips name the installed mesh/tmux fact. No new skip is authorized. A failed
wait remains a failure. Failure
screenshots can show cleanup's disband, so diagnosis must use the preceding
app log, as documented by `673dac42`.

## Test lanes

| Recipe | What it runs |
|--------|-------------|
| `just test` | All non-E2E tests (Rust + frontend) |
| `just test-fast` | Rust compile-check + frontend Vitest |
| `just test-rust-fast` | Cargo test compile check |
| `just test-rust-unit` | Rust unit tests (no daemon/network) |
| `just test-rust-integration` | Every `src-tauri/tests/*.rs` system/integration test binary plus the heavy `--lib` suites; system binaries and shared-global subsets are serialized, while daemon server/client fixture suites use default parallelism |
| `just test-frontend` | Vitest frontend tests |
| `just test-visual` | Browser-mode visual screenshot lane |
| `just visual-shot C S [V] [T] [OUT]` | One fixture shot at window size via Edge headless |
| `just visual-shot-stop` | Stop the visual host `visual-shot` started |
| `just test-daemon-connectivity` | Manual daemon connectivity chain checks |
| `just test-e2e` | Tier 1 E2E |
| `just test-e2e-full` | Tier 1 + Tier 2 E2E |
| `just test-e2e-spec SPEC` | Single E2E spec |
| `just test-e2e-spec compaction-codex-hooks` | Paid Codex compaction lane (never in a suite run) |
| `just test-e2e-spec managed-stage-codex` | Paid managed Codex stage lane (never in a suite run) |
| `just test-e2e-spec managed-stage-deadline` | Paid managed stage deadline lane (never in a suite run) |
| `just test-e2e-spec managed-stage-parallel` | Paid parallel managed-stage isolation lane (never in a suite run) |
| `just test-macos` | Rust tests on remote Mac Mini |
| `just test-macos-e2e` | macOS E2E on remote Mac Mini |
| `just agent-quality` | Agent-facing wrapper around `just check-quick` |

### CI schedule

| Job | Command | When it runs |
|-----|---------|--------------|
| `Rust unit tests` | `just test-rust-unit` | Every pull request, main push, and manual workflow dispatch |
| `Rust integration tests` | `just test-rust-integration` | Every pull request, main push, and manual workflow dispatch |

Both Rust jobs cache build artifacts, including failed builds for faster retries, without skipping test execution. The Rust-only lanes need Cargo, `just`, and the Linux/Tauri system libraries installed by the workflow; the current recipe's tmux interactions use a fake executable, while its Git fixtures use libgit2 with explicit signatures.

`just test-rust-integration` runs **every** binary in `src-tauri/tests` — all 11 of them — because the recipe derives its `--test` arguments from a `justfile` variable that globs `src-tauri/tests/*.rs` rather than from a hand-kept list (`justfile:9`). A guard in `src-tauri/tests/module_boundary_assertions.rs` evaluates that variable and fails the build if the derived manifest and the directory disagree in either direction, and a second guard requires both Rust lanes to source the heavy-suite filters from the same `heavy_rust_test_filters` variable (`justfile:10`), so a heavy suite the unit lane skips is a heavy suite the integration lane re-runs.

The daemon server and daemon-client fixture suites run with Cargo's default
parallel test runner. Their shared fixtures bind one ephemeral listener and
retain that exact socket across the serving-thread handoff, closing the former
drop-and-rebind race. Heavy tests that exercise true process-global or watcher
state remain behind named guards or the lane's explicit serial branch.

### Bisection recipes

When a test failure needs narrowing down:

```bash
just test-rust-bisect-unit          # Bisect unit tests by module
just test-rust-bisect-heavy         # Bisect daemon/network tests
just test-rust-bisect-commands      # Bisect commands module
just test-rust-bisect-coordination  # Bisect coordination module
```

## Verification gates

```bash
just check-quick   # Per-task fast gate
just check         # Full gate (team-lead serialized runs or pre-release)
```

`just check-quick` runs:
1. `cargo fmt` — Rust format auto-fix
2. `cargo check --tests` — Rust compile + test-target validation
3. `bun run typecheck` — Svelte type checking
4. `bun run test` — Frontend unit tests

`just agent-quality` delegates to `just check-quick` and exists as the explicit pre-completion gate for agent workflows.

`just check` runs `just fmt` first, then two lanes in parallel and joins on every lane's status — the first non-zero exit kills the other lane and fails the gate:

| Lane | Steps |
|---|---|
| Rust | `just lint-rust` (clippy), `just test-rust` (compile check + unit + integration) |
| Frontend | `just lint-frontend`, `just lint-workflows`, `just typecheck`, `just test-frontend` |

Full output is tee'd to `.check-logs/check-<timestamp>.log` (override the directory with `TAURHAUS_CHECK_LOG_DIR`) and only the five newest logs are kept. `just lint` is a superset of the *lint* steps above and of nothing else: it runs `lint-rust`, `lint-frontend`, `lint-workflows` and adds `lint-just-gates`, which re-runs the real lane joiner against seeded failures to prove `just check` still fails closed (`justfile:167`). It is not a broader gate than `just check` — it never runs `fmt`, `typecheck`, `test-rust` or `test-frontend`, so it is no substitute for the full gate.

**Run `just check-quick` on every task.** In team/agent workflows, agents should not run `just check`; team-lead owns serialized full-gate runs.

E2E tests run at milestones, not on every task.

## Regression testing

Every regression fix ships with a corresponding test. This is non-negotiable.

### Where regression tests go

| Layer | Location | Format |
|-------|----------|--------|
| E2E | `e2e/specs/regressions.js` | One `describe` block per regression |
| Rust | Affected module's `#[cfg(test)]` | `#[test]` with `// Regression:` comment |
| Frontend | Affected module's `.test.js` | Test case with `// Regression:` comment |

### What to document

Every regression test must include:
1. **What broke** — the visible symptom
2. **Which commit broke it** — the offending change
3. **Why** — root cause explanation

Example:
```rust
#[test]
fn session_file_dedup_rejects_duplicate_path() {
    // Regression: duplicate session imports caused sidebar duplication
    // Commit: abc1234 — removed unique index during migration refactor
    // Root cause: migration 002 was skipped when running from clean DB
    ...
}
```

## Visual review

Frontend tasks undergo visual review using 8 categories, each scored 1–10 with a minimum of 9 per category.

**Dual review process**:
1. Self-review by the implementer
2. Cross-review by the other model family (screenshot analysis), the same Opus ↔ Codex pairing every PR review loop uses
3. Lower score wins; the orchestrator is final arbiter with justified override

This applies to frontend tasks only — backend tasks skip visual review.

## Key files

| File | Purpose |
|------|---------|
| `justfile` | All test recipes and verification gates |
| `e2e/README.md` | E2E runbook and troubleshooting |
| `e2e/specs/regressions.js` | E2E regression test suite |
| `vitest.config.ts` | Frontend unit test configuration |
| `vitest.visual.config.js` | Browser-mode visual test configuration |
| `scripts/visual-shot.sh` | Edge-headless window-size screenshot lane |
| `src/visual-host/query.js` | URL → fixture address for the visual host |
| `e2e/wdio.conf.js` | WebdriverIO configuration |
| `scripts/rust-test-bisect.sh` | Rust lane/module bisect helper |

## Related documents

- [CLAUDE.md](../../CLAUDE.md) — TDD policy, quality gates, regression testing rules
- [visual-testing-guide.md](./visual-testing-guide.md) — manual visual host and screenshot lane details
- [Build and release](build-and-release.md) — build recipes and release workflow
