# Hardening milestone before W4 — plan and ledger

Status: **approved 2026-08-30** (all three phases as ordered); Track A (evidence) complete; Phase 1 in progress — 1a (#82), 1b+1c (#81) merged, 1d and 1e in flight. Precedes W4 (`w4-managed-stages-design.md`, experiments 4–5 and the implementation they gate). Companion to [`workflows-integration-plan.md`](workflows-integration-plan.md) (procedures) and [`accounts-and-usage-plan.md`](accounts-and-usage-plan.md).

## Why now

Two weeks of feature work — 687 commits, 77 PRs, +102k/−33k lines — grew the codebase to 141k lines of Rust and 40k of frontend, with six files receiving 26–48 commits each. Today's review rounds on the account-picker fixes (#74, #75) found seam defects rather than feature logic: a whitelisting IPC normalizer silently dropping a new backend field, Settings re-deriving a precedence the backend owns, a hard-coded RPC timeout, and flaky tests in files nobody touched. W4 lands in `coordination/`, the largest and most test-heavy area. The milestone is bounded: evidence first, exit criteria per phase, then W4.

## How the evidence was produced (Track A, 2026-08-30)

1. **Research sweep** — six areas (coordination, harness model, commands/IPC/wire, daemon/lifecycle/startup, frontend, tests/gates/procedures) × two lenses: Codex gpt-5.6-sol wrote a structured survey with a cost attached to every item; Opus read each area adversarially. Twelve reports (the tests/procedures survey needed a second, analysis-only run after the first spent its budget timing lanes).
2. **Consolidation** — Codex merged the twelve reports into 26 candidates, 36 do-not-touch constraints and 8 dropped items; the orchestrator added three procedure items from the re-run survey.
3. **Judge panel** — for each of 29 candidates, three independent Opus judges: an evidence check (every cited `file:line` opened), a risk-to-W4 lens (against the design note), and a cost/preservation lens (what tests must exist before the change). Ranking is deterministic: `score = (risk + W4 exposure) / fix cost`; a candidate counts as **verified** only when ≥2 of 3 judges confirmed both the evidence and the cost claim. 27 of 29 verified.

Model roles, revised by the operator on 2026-08-30 from the two-week ledger: **Fable** orchestrates (specs, reshape and merge calls, arbiter); **Codex gpt-5.6-sol** implements, refactors and writes surveys; **Opus** reviews adversarially (conformance + operational lenses on a `feature-pr`, one lens on a `small-change`) and checks behaviour preservation. Whoever implements never reviews. Codex lanes are sized one module at a time.

## Already landed or in flight (pulled ahead of the ranking)

| Item | Why it could not wait | Where (both merged 2026-08-30) |
|---|---|---|
| `just check` exits 0 when a lane fails | The full gate could not fail; every release gate before today proved nothing on the failure path (the 0.8.4 gate did pass for real — the "Full quality gate passed." line prints only on all-green) | #80 — fix plus a seeded-failure guard in `just lint` |
| Lossless IPC normalizers | `normalizeSettings` dropped `default_account_ids`, so any settings save (a theme toggle included) **wiped the user's default accounts** in the shipped app; `normalizeCoordinationMember` dropped `task_effort`, so the W5b chip never rendered from real data | #78 — pass-through normalizers + Rust-generated JSON goldens fed through each one |

## The ranked table

Scores from the judge panel; risk 0–5 (5 = bites within two weeks), fix cost 1–5 (5 = several lanes with wire/docs consequences), W4 0–3 (3 = W4 lands on top of it), behaviour risk 0–3 (risk the fix itself changes behaviour).

| # | Candidate | Area | Size | Risk | Fix cost | W4 | Behaviour risk | Score | Verified |
|---|---|---|---|---|---|---|---|---|---|
| 1 | `review-lens-authority-question` — Add the authority-ownership question to both code-review lenses | tests-procedures | small-change | 2.7 | 1.0 | 1.7 | 1.3 | 4.33 | 3/3 ev · 3/3 cost |
| 2 | `open-major-handoff-outcome` — Make an open-major handoff an explicit non-complete outcome | tests-procedures | small-change | 2.3 | 2.0 | 2.0 | 2.0 | 2.17 | 3/3 ev · 3/3 cost |
| 3 | `coordination-presentation-status-contract` — Pin one coordination presentation, progress, and status vocabulary | cross-cutting | feature-pr | 4.0 | 3.7 | 3.0 | 2.0 | 1.91 | 3/3 ev · 3/3 cost |
| 4 | `pure-w4-deadline-policy` — Fence W4 deadline self-heal into a pure injected-clock policy | coordination | small-change | 2.0 | 2.3 | 2.3 | 1.0 | 1.86 | 3/3 ev · 2/3 cost |
| 5 | `rust-gate-truth-completeness` — Make Rust gate exit status, execution manifest, CI coverage, and bisection truthful | tests-procedures | feature-pr | 3.7 | 3.7 | 3.0 | 1.7 | 1.82 | 3/3 ev · 3/3 cost |
| 6 | `delivery-wake-outcomes` — Distinguish durable delivery from wake and post-write outcomes | coordination | feature-pr | 2.3 | 2.7 | 2.3 | 1.0 | 1.75 | 3/3 ev · 3/3 cost |
| 7 | `assignment-effort-contract` — Extract and make assignment-effort gating task-specific and observable | coordination | feature-pr | 3.3 | 3.7 | 3.0 | 2.0 | 1.73 | 3/3 ev · 3/3 cost |
| 8 | `coordination-runtime-transaction-schema` — Make coordination runtime writes transactional, schema-preserving, and ownership-safe | coordination | feature-pr | 4.0 | 4.3 | 3.0 | 2.7 | 1.62 | 3/3 ev · 3/3 cost |
| 9 | `test-e2e-external-state-isolation` — Isolate every test/E2E home, token, tmux server, process, and spec manifest | tests-procedures | feature-pr | 4.0 | 4.3 | 3.0 | 2.7 | 1.62 | 3/3 ev · 3/3 cost |
| 10 | `lossless-frontend-response-contracts` — Make exposed frontend response normalization additive-safe | cross-cutting | feature-pr | 3.0 | 3.0 | 1.7 | 2.0 | 1.56 | 3/3 ev · 3/3 cost |
| 11 | `managed-harness-launch-policy` — Consolidate managed-harness launch, shell, effort, and hook policy below commands | cross-cutting | feature-pr | 3.7 | 4.7 | 3.0 | 2.3 | 1.43 | 3/3 ev · 3/3 cost |
| 12 | `role-contract-end-to-end` — Use one field-complete role and behavioral-contract adapter end to end | cross-cutting | feature-pr | 4.0 | 4.0 | 1.7 | 2.0 | 1.42 | 3/3 ev · 3/3 cost |
| 13 | `startup-orchestration-telemetry` — Test the production startup path and report degraded/failing stages honestly | daemon | feature-pr | 2.3 | 3.0 | 1.3 | 2.0 | 1.22 | 3/3 ev · 3/3 cost |
| 14 | `typed-workflow-gate-catalog` — Replace prose/substrings with a typed workflow gate catalog | tests-procedures | feature-pr | 2.3 | 4.0 | 2.3 | 2.3 | 1.17 | 3/3 ev · 3/3 cost |
| 15 | `checked-daemon-compatibility-windows-snapshot` — Use one checked daemon compatibility contract, including Windows snapshots | daemon | feature-pr | 2.7 | 4.3 | 2.3 | 2.3 | 1.15 | 3/3 ev · 3/3 cost |
| 16 | `dead-boundary-contract-cleanup` — Prune dead boundary APIs and enforce declaration-to-caller parity | cross-cutting | small-change | 2.3 | 3.7 | 1.7 | 1.7 | 1.09 | 3/3 ev · 3/3 cost |
| 17 | `test-global-context-scanner-isolation` — Unify Rust test globals, environment restoration, and scanner pane seams | tests-procedures | feature-pr | 2.3 | 3.7 | 1.7 | 1.3 | 1.09 | 3/3 ev · 3/3 cost |
| 18 | `daemon-worker-raii-recovery` — Give daemon watch, usage, and long-lived workers RAII recovery and supervision | daemon | feature-pr | 2.3 | 4.0 | 2.0 | 2.3 | 1.08 | 3/3 ev · 3/3 cost |
| 19 | `test-module-move-only-split` — Split giant Rust test modules only in an isolated move-only lane | tests-procedures | feature-pr | 1.3 | 4.0 | 2.7 | 1.3 | 1.00 | 3/3 ev · 2/3 cost |
| 20 | `harness-session-identity-classification` — Harden harness session identity and non-session classification | harness | feature-pr | 2.3 | 4.3 | 2.0 | 2.3 | 1.00 | 3/3 ev · 3/3 cost |
| 21 | `authoritative-account-resolution` — Make backend account preview the only effective-account authority | cross-cutting | feature-pr | 3.0 | 4.0 | 1.0 | 3.0 | 1.00 | 3/3 ev · 3/3 cost |
| 22 | `method-typed-daemon-rpc` — Give daemon RPC methods typed timeouts, idempotency, results, IDs, and errors | wire | feature-pr | 3.0 | 4.3 | 1.3 | 3.0 | 1.00 | 3/3 ev · 3/3 cost |
| 23 | `harness-registry-conformance` — Make harness registry, descriptors, bindings, and capability invariants fail closed | harness | feature-pr | 2.0 | 4.0 | 1.7 | 1.0 | 0.92 | 3/3 ev · 3/3 cost |
| 24 | `daemon-server-single-listener-runtime` — Bind daemon test/production listeners once and own each server runtime | daemon | feature-pr | 2.7 | 4.0 | 1.0 | 2.3 | 0.92 | 3/3 ev · 3/3 cost |
| 25 | `frontend-theme-contract-guard` — Enforce no-new inline theme branches and fix muted-text contrast | frontend | small-change | 1.7 | 3.3 | 1.3 | 2.3 | 0.90 | 3/3 ev · 2/3 cost |
| 26 | `popup-keyboard-focus-contract` — Unify account and popup keyboard/focus semantics | frontend | feature-pr | 2.0 | 3.3 | 0.0 | 2.3 | 0.60 | 3/3 ev · 3/3 cost |
| 27 | `harness-module-decomposition` — Decompose harness models and account core behind stable re-exports | harness | feature-pr | 1.3 | 4.0 | 1.0 | 1.7 | 0.58 | 3/3 ev · 3/3 cost |
| 28 | `workflow-lib-drift-lint` — Teach the workflow-script lint to reject shared-library drift | tests-procedures | small-change | 1.0 | 1.3 | 2.7 | 0.7 | 2.75 | 0/3 ev · 0/3 cost |
| 29 | `frontend-state-lifecycle-cleanup` — Close frontend activity, project-reset, and workflow-store lifecycle leaks | frontend | small-change | 2.0 | 2.7 | 1.3 | 2.0 | 1.25 | 0/3 ev · 3/3 cost |

Two candidates failed verification: `workflow-lib-drift-lint` is refuted — the byte-identity guard already exists in `scripts/workflow-procedures.test.mjs:99-113`; `frontend-state-lifecycle-cleanup` has a real cost but its citations did not match (0/3 evidence) and is re-verified before any lane.

One item surfaced *during* the normalizer pass and was deliberately kept out of that reviewed PR: `invokeOrMock` (`src/lib/ipc/client.js`) returns the `invoke` promise without `await`, so a Tauri rejection bypasses its `try/catch` and `normalizeInvokeError` never runs on the real path. It joins Phase 3 as a `small-change` (one word, one test, every IPC caller's error shape changes — it needs the lens).

## Lane order

The order is not the score order alone: gate truth comes first because every later lane is measured by it; the W4 blockers come next because W4 writes into exactly that file and that pass; the rest follows the score. Each lane is one procedure run (`small-change` or `feature-pr`), Codex implementing, Opus reviewing, with the behaviour-preservation tests the judges listed as *missing* written **before** the change.

### Phase 1 — gates and procedures that tell the truth (≈ half a day)

| # | Lane | Procedure | Notes from the judges |
|---|---|---|---|
| 1a | `rust-gate-truth-completeness` (execution manifest) | small-change | An explicit manifest of the integration binaries with a static completeness test; `harness_conformance` (20 tests) and three other binaries are not run by any recipe today. Separable from the in-flight status fix and from CI. |
| 1b | `review-lens-authority-question` | small-change | Only **three** scripts carry a review lens (`small-change`, `fix-round`, `feature-pr` ×2), not five; nothing pins lens text — add a prompt-content assertion per review lane in `scripts/workflow-procedures.test.mjs` and a cross-script sync assertion (the lenses live outside the byte-identical lib block); `feature-pr` re-runs only the conformance lens in fix rounds, so the question goes into both. |
| 1c | `open-major-handoff-outcome` | small-change | `small-change`/`fix-round` return an unconditional completed ledger with `remaining` majors; add an explicit non-complete outcome and a stubbed-run test whose last review still files a major. |
| 1d | `rust-gate-truth-completeness` (CI executes Rust) | feature-pr | A serialized Rust test job in `quality-gate.yml` after 1a and the status fix; CI runs no Rust test today. Budget is the argument to settle in the spec. |
| 1e | `typed-workflow-gate-catalog` | feature-pr | `args.gates` as an array of exact commands (or rejected), required commands matched exactly not by substring, a Rust test run required for Rust diffs. Today's hand-written gate line with invalid cargo syntax cost a gate round. |

### Phase 2 — the coordination seams W4 lands on (≈ 1.5 days)

| # | Lane | Procedure | Notes from the judges |
|---|---|---|---|
| 2a | `coordination-runtime-transaction-schema` | feature-pr | **W4 blocker.** The 30 s self-heal pass runs on a second orchestrator outside the mutex every IPC path takes (commit 366f4b7 removed the exclusion to fix a Windows freeze and replaced it with nothing); `MemberRuntimeRecord` is a closed struct in a file mesh also writes (`appliedEffort` whitelisted by hand). Hold the existing per-team lock across load→decide→save; add a flattened extension map; tolerant decoders; pane identity preserved on probe failure. No concurrency test above the store layer exists — write the two-orchestrator interleaving test and the unknown-key round-trip first. |
| 2b | `assignment-effort-contract` | feature-pr | Extract the ~330-line pending-effort/launch-rewrite/retry block from member activation into its own module; carry the held task id; typed switched/failed outcomes. 16 effort tests already pin most behaviour — keep them. |
| 2c | `delivery-wake-outcomes` | feature-pr | `DeliveryResult` carries no durability/wake/warning distinction; post-write failures are logged and the unmodified backend result returned. Add one-append assertions for the seven uncovered outcomes before changing anything. |
| 2d | `pure-w4-deadline-policy` | small-change | A narrow injected-clock deadline transition with persisted one-shot markers, fenced from the placeholder health framework (`health/transition.rs` is an identity placeholder with no call site). Pins the self-heal pass's current decisions first. |
| 2e | `coordination-presentation-status-contract` | feature-pr | Live and fast snapshots carry the same 19 fields through two near-identical constructors; a shared Rust/JS state vocabulary with unknown states mapped to `uncertain`. Depends on the normalizer PR. |

### Phase 3 — host safety, launch policy, daemon ownership (≈ 1.5 days)

| # | Lane | Procedure | Notes from the judges |
|---|---|---|---|
| 3a | `test-e2e-external-state-isolation` | feature-pr | An E2E run can reconcile hook installers into the operator's real `~/.gemini` (`agy_dir()` has no override), `~/.grok`, `~/.codex`; the daemon token lives under a root `TAURHAUS_DATA_DIR` does not cover; pre-run cleanup kills by process pattern host-wide. Route every root through `PlatformPaths`, isolate all four tool roots per worker, own PIDs only. |
| 3b | `managed-harness-launch-policy` | feature-pr | The team launch path never resolves the base command through the pane shell (the alias seam #75 built is app-launch only; `coordination` cannot import `commands` by boundary test), and shell-word parsing is duplicated three times. Team-path byte goldens first — only one exists. |
| 3c | `lossless-frontend-response-contracts` (remainder) | small-change | The role editor round-trip and the descriptor allowlist (`CliCapabilityDescriptor` already drops `agent_definitions`/`workflow_runs`). |
| 3d | `invokeOrMock` awaits `invoke` | small-change | See above. |
| 3e | `role-contract-end-to-end` | feature-pr | Preset and direct-role hydration omit seven fields both sides declare; one field-complete adapter. |
| 3f | `checked-daemon-compatibility-windows-snapshot` | feature-pr | Startup demands exact protocol + version while runtime health checks protocol only; the Windows snapshot path bypasses configured daemon state. |
| 3g | `startup-orchestration-telemetry`, `daemon-worker-raii-recovery`, `daemon-server-single-listener-runtime`, `method-typed-daemon-rpc` | feature-pr each | Daemon ownership and wire typing; the server flake (`server_handles_file_tree`) is root-caused to reserve-drop-rebind plus a 750 ms unbound window — the launcher already has the fix pattern. |

### Deferred (score below 1.1, or W4 exposure 0)

`dead-boundary-contract-cleanup`, `test-global-context-scanner-isolation`, `test-module-move-only-split` (the judges agree with the surveys: do not split the giant test files for size alone), `harness-session-identity-classification`, `authoritative-account-resolution` (high behaviour risk 3.0 — revisit after 3b/3c, since the backend preview must be the one authority first), `harness-registry-conformance`, `frontend-theme-contract-guard`, `popup-keyboard-focus-contract`, `harness-module-decomposition`. They stay on this list with their evidence; nothing here is forgotten, only sequenced after W4.

### Track C

A `docs-sweep` after Phase 3, since CLAUDE.md, ARCHITECTURE.md and the architecture references were patched incrementally for two weeks.

## Exit criteria

- `just check` fails on a failing lane (guarded), CI executes Rust tests, every integration binary is in a manifest with a completeness test, procedure gates are typed and matched exactly.
- Coordination runtime writes are one critical section per team, unknown mesh-owned keys survive a round-trip, the deadline policy is pure and injected — the three facts W4's experiments 4–5 need.
- An E2E run cannot touch a real tool root or a foreign process.
- Every lane green under the procedures with Opus review; the milestone ledger below filled at merge.

## Ledger

| Lane | Implementer | Reviewers | Rounds | Majors found | Merged |
|---|---|---|---|---|---|
| `just check` lane status | Codex gpt-5.6-sol | Opus ×1 | 3 (small-change + 1 fix-round) | 2 (seeded runs evicting the real gate logs; a tee-flush race making the guard itself flaky under load) → 0; one minor — the `kill -0` prune dropping a lane that exited non-zero in the reap gap — closed in the orchestrator's pass with the reviewer's counter-based join | #80 |
| lossless IPC normalizers | Codex gpt-5.6-sol | Opus ×1 | 1 | 0 (1 minor + 4 nits, orchestrator's pass); the settings round-trip was proven a wipe, not an error | #78 |
| 1a integration manifest | Codex gpt-5.6-sol | Opus ×1 | 2 | 1 (the justfile-reading guards panicked under a bare `cargo test` without `just`) → 0; one minor (a `$`-prefixed filter bypassing the parity guard) closed in the orchestrator's pass. All eleven binaries ran: 101 s | #82 |
| 1b+1c procedure honesty | Codex gpt-5.6-sol | Opus ×1 | 1 | 0 (1 minor — the `followup` call was not runnable as handed back — + 3 nits, orchestrator's pass) | #81 |

## Do not touch (union of the reports, carried by the judges)

- Coordination store lock order, team lock, target-file lock, atomic rename, inode re-check, and retry numbers — they are a matched cross-process correctness protocol with prior visibility/lost-effort regressions; extend preservation under the same mechanism (src-tauri/src/coordination/stores/lock.rs:17-21,85-216; stores/runtime.rs:99-161).
- Existing #[serde(flatten)] extra maps on TeamConfig, Member, and MeshInboxMessage, plus config/inbox ownership and file locations — they already preserve mesh-owned fields; add equivalent runtime protection rather than remove/move them (stores/config.rs:43-76,139-176; stores/inbox.rs:19-41; docs/architecture/data-architecture.md:195-207).
- The three-attempt effort budget and RuntimeEffort SlashCommand/ResumeWithFlag split — the budget prevents repeated pane teardown and the split prevents taurhaus typing effort into another harness's live pane (pipelines/members.rs:38-43,423-428; pipelines/tests.rs:3820-3866).
- Task correlation by owned task rather than newest inbox message — commit 5384985's regression protects against the wrong heuristic; multi-assignment work must add task identity without restoring inbox correlation (operational_context.rs:192-227,478-531).
- pane_belongs_to_member, same-observation quarantine CAS, session-id-first compaction matching, and MemberRuntimeStore::update acquire_if_exists — these are the strong/safe halves; extend call sites and inputs, never weaken them (runtime/mod.rs:247-345; compact_hook.rs:779-799; stores/runtime.rs:165-198).
- Compaction/hook behavior, installer layout, and accounts/legacy_statusline.rs before W4 — they mutate multi-platform user config and have high regression radius; add only non-invasive conformance guards now (coordination/compact_hook.rs:190-375,637-1555; accounts/legacy_statusline.rs:9-149).
- A general health/recovery framework — W4 needs a narrow injected-clock task deadline policy, not completion of the explicit placeholder framework (docs/coordination-architecture.md:127-148).
- Fast snapshot no-probe semantics — W4 fields must come from persisted task/operational data, never new tmux/WSL probes in the snapshot IPC (docs/coordination-architecture.md:317-327).
- Coordination backend trait/selector, mesh floor, and config/inbox ownership — selection is already capability-driven and W4 is not a third backend (coordination/backend/selector.rs:25-45; docs/coordination-architecture.md:44-50).
- Tmux/daemon/store/hook waits, retry counts, and production timeouts merely while centralizing policy — preserve values until a failing behavior test or telemetry justifies tuning (coordination/runtime/mod.rs:36-43; launch_base.rs:373-450; control.rs:318-359; daemon_lifecycle.rs:603-648).
- Automatic reruns, cross-machine stages, workflow-launch UI, and unrelated Claude-side changes — W4 explicitly excludes them (docs/design/w4-managed-stages-design.md:34-35).
- The central per-tool registry records and capability-slice architecture — keep the auditable rows together; do not distribute them beside providers or introduce a generic adapter hierarchy (docs/architecture/harness-model.md:13-31; cli_tool.rs:241-608).
- Tool-identity allowlists/boundaries — do not widen allowed files or raise ceilings merely to land W4; narrow after approved module splits (src-tauri/tests/module_boundary_assertions.rs:243-307).
- Launch/onboarding/agent-definition golden bytes and renderer deny_unknown_fields/multiline rejection — they are execution-safety and operator-pane contracts, not persisted round trips (src-tauri/tests/harness_conformance.rs:83-109; src-tauri/tests/cli_renderers.rs:246-385; src-tauri/src/lib.rs:332-375).
- AccountOrigin wire names and resolve_launch_account precedence — frontend fixes must consume this authority, not align another copy (accounts/mod.rs:48-62,131-249,1483-1497).
- Cross-target/test cfg allow markers in daemon/process/launcher/store re-exports — reports found no concrete W4 cost and they preserve non-host compilation seams (session_scanner/daemon.rs:11-19; process.rs:80-155; daemon/launcher.rs:69-82).
- Retired Gemini aliases, Codex transcript compaction, and Claude status-line usage — persisted-unknown and retirement behavior is deliberate (docs/architecture/harness-model.md:105-110).
- CliVersions/ModelCatalog conversion to generic maps solely for aesthetics — a W4 version-gate need is UNVERIFIED; preserve current wire shapes through any split (models/mod.rs:330-407; src/lib/ipc/system.js:115-150).
- The daemon exact-version gate, serde(default) compatibility markers, UNKNOWN_METHOD behavior, and one-repair-per-episode semantics — reconcile toward the strict gate; do not relax it or remove defaults to surface drift (daemon/protocol.rs:13-35,398-450; daemon/handlers.rs:92-96; daemon_lifecycle.rs:888-963).
- Provider path normalization and its golden corpus — it is the central Windows/WSL/native authority and no missing path form was found (provider/platform_paths.rs:17-139; provider/path.rs:148-178,581-663).
- Daemon/mesh installer and launch-environment behavior while changing RPC/compatibility — incident regressions protect exact pairing, repair order, log/data roots, and intentional WSL parent behavior (commands/daemon.rs:869-900,1045-1075; daemon/launcher.rs:579-588,695-759).
- Tmux focus ownership — keep the daemon session hub as the sole probe and lifecycle as transport; never resurrect hook/file/inotify focus (daemon/session_activity.rs:239-275; daemon_lifecycle.rs:1638-1766).
- Watcher classification, directory pruning, shared refcounts, git debounce, and event-processor batching — log/recover ownership failures without rewriting shared local/daemon semantics (fs/watcher.rs:135-576,1453-1481; daemon/watch.rs:11-15; event_processor.rs:449-534).
- Auth tokens, credentials, usage payload contents, endpoints, and refresh semantics beyond worker ownership — never log/persist secrets or fold security-sensitive provider redesign into structural work (daemon/auth.rs:27-51,134-151; daemon/usage_poller.rs:188-284).
- activitySignal evidence fields and the 60-second workflow-write window — they are the established single cross-surface truth and prior divergence guard (src/lib/activitySignal.js:1-53,112-225).
- Mesh route geometry and FilesTab consume-after-capture — both exactly follow the documented Svelte/mesh ownership rules (src/lib/components/meshLayout.js:365-417; MeshCanvas.svelte:409-448; FilesTab.svelte:134-146).
- Existing bounded polling architecture and test reset seams — do not change account retry deadlines, workflow reference-counting/poll shape, session fallback, mesh no-overlap polling, or remove deterministic reset exports (accounts.svelte.js:172-265,468-480; workflowRunStore.svelte.js:91-198; meshTabRuntime.svelte.js:478-520).
- Wholesale MeshTeamBuilder/MeshNodeDetail moves, repository-wide theme rewrites, global CSS deletion, ContextMenu geometry, or the working MeshRuntimeBar menu — isolate only contract/focus changes with visual equivalence (MeshTeamBuilder.svelte:912-1095; ContextMenu.svelte:128-166,249-256; MeshRuntimeBar.svelte:135-219).
- Screenshot/golden outputs during test-file movement — keep existing visual evidence; do not mask a structural move by refreshing baselines (docs/operations/testing-guide.md:9-14).
- --test-threads=1, heavy-suite skip/re-run split, heavy/env/log guards, and fail-closed workflow/reviewer behavior until global-state consolidation is proven — they contain real shared-state coupling (justfile:195-213; src-tauri/src/test_support.rs:5-105; scripts/workflow-procedures.test.mjs:400-597).
- Visual, E2E, paid lanes, metrics, daemon installation, real CLIs, and real tool homes in ordinary CI — preserve paid-spec exclusion, scratch roots, opt-in daemon install, maxInstances=1, owed cleanup, and build-order warnings (e2e/specList.js:27-42; e2e/wdio.conf.js:557-633; e2e/README.md:42-58).
- The giant coordination test files during any behavioral hardening change — reports disagree on splitting; if done, use a later isolated move-only lane after gate fixes, never mix it with W4 logic (coordination/orchestrator/tests.rs:1; pipelines/tests.rs:1; commands/coordination/tests.rs:1).
- The daemon server's 750 ms initial hub wait — it seeds the first snapshot; fix listener ownership instead of deleting the wait to hide the race (daemon/server.rs:103-113).
- mark_disconnected full-pool teardown until method timeouts and response-ID validation are fixed — current teardown accidentally prevents stale-response reuse (provider/daemon_client.rs:475-488).
- Workflow small-change fix-round cap and fail-closed verdict logic — add typed gates/structured handoff, never auto-approve, discard remaining findings, or make the loop unbounded (.claude/workflows/small-change.js:608-687; fix-round.js:407-415).
- Semantic response/request boundary distinctions — do not replace write payload validation or executable deny-unknown contracts with indiscriminate pass-through merely to fix response field loss (src/lib/ipc/coordinationResponses.js:146-193; src-tauri/src/lib.rs:332-375).

## Appendix — every candidate in one line

- **`review-lens-authority-question`** — The conformance and operational review prompts do not ask whether frontend code re-derives backend-owned truth or whether several views bypass one authority; three of today's majors on #75 were exactly that class and were found late. _Raised by:_ tests-procedures-survey-r2, frontend-adversarial.
- **`open-major-handoff-outcome`** — small-change (one fix round) and fix-round return a completed ledger with `remaining` majors; nothing mechanical stops a merge on top of open majors — today the chooser lane's open majors were caught only because the orchestrator read the ledger. _Raised by:_ tests-procedures-survey-r2.
- **`coordination-presentation-status-contract`** — Live and fast agent shapes duplicate fields and constructors, frontend normalization drops task effort, two StepProgressEvent contracts differ, and unknown session states currently become offline. _Raised by:_ coordination-survey, wire-survey, wire-adversarial, frontend-adversarial, tests-procedures-survey.
- **`pure-w4-deadline-policy`** — The existing health transition module is an identity placeholder, while W4 needs exactly-once nudge-at-half and stale-at-deadline behavior. _Raised by:_ coordination-survey.
- **`rust-gate-truth-completeness`** — just check captures status after negation and can exit zero on a failed lane; CI/check-quick execute no Rust tests; four integration binaries are absent from just test; substring bisection selects unrelated tests. _Raised by:_ harness-survey, tests-procedures-survey, tests-procedures-adversarial.
- **`delivery-wake-outcomes`** — After an inbox append, wake, operational-context persistence, and runtime-state updates are best-effort warnings, but callers receive the same nominal delivery success. _Raised by:_ coordination-survey.
- **`assignment-effort-contract`** — The Codex effort relaunch state machine is embedded in member activation, chooses one latest task while mesh gates the task being delivered, collapses team/config failures to zero work, and becomes silent after the three-attempt budget is exhausted. _Raised by:_ coordination-survey, coordination-adversarial.
- **`coordination-runtime-transaction-schema`** — Foreground IPC, background self-heal, activation, liveness, and mesh can read-modify-write the same member runtime document without one transaction; the Rust wire is closed, preserves only appliedEffort by name, can reject mesh-created minimal records, [...] _Raised by:_ coordination-survey, coordination-adversarial.
- **`test-e2e-external-state-isolation`** — The daemon token ignores TAURHAUS_DATA_DIR, ordinary E2E does not isolate Codex/Grok/Agy roots, several tmux-driving specs use the operator server, fallback cleanup uses broad pkill patterns, and unpaid specs can accumulate in one catch-all. _Raised by:_ daemon-adversarial, tests-procedures-survey, tests-procedures-adversarial.
- **`lossless-frontend-response-contracts`** — Several JavaScript response normalizers reconstruct fixed whitelists, already dropping the default-account map and task-effort fields and leaving future descriptor/template additions silent. _Raised by:_ harness-survey, wire-survey, wire-adversarial, frontend-survey, tests-procedures-survey, tests-procedures-adversarial.
- **`managed-harness-launch-policy`** — Team launches bypass pane-shell base resolution, three Rust paths plus Settings implement shell-word/assignment rules, LaunchSpec's four arms have drifted, and managed hook/launch policy lives in commands/terminal_settings where coordination cannot import it. _Raised by:_ harness-survey, harness-adversarial, wire-survey.
- **`role-contract-end-to-end`** — Backend preset/direct-role hydration omits seven known steering fields, while frontend role editors flatten or recategorize behavioral contracts and the role response whitelist has unguarded fields. _Raised by:_ coordination-survey, wire-survey, frontend-survey, tests-procedures-adversarial.
- **`startup-orchestration-telemetry`** — Startup order tests exercise a test-only copy, optional hook/search/background failures are log-and-continue or silently fall back, and completed events can be emitted without truthful degraded/failure fields. _Raised by:_ daemon-survey, daemon-adversarial.
- **`typed-workflow-gate-catalog`** — Five workflow scripts duplicate gate prose and accept reported command strings by substring, so invalid Cargo syntax or an echo containing a required phrase can consume or falsely satisfy a gate; review lenses omit backend-authority/normalizer questions. _Raised by:_ tests-procedures-survey, tests-procedures-adversarial.
- **`checked-daemon-compatibility-windows-snapshot`** — Windows session snapshots open a raw fixed-port socket with no protocol check, while startup checks protocol plus package version and runtime health/bridge paths apply different pairing truth; reports also disagree whether the current optional workflow [...] _Raised by:_ harness-survey, wire-adversarial, daemon-survey, daemon-adversarial.
- **`dead-boundary-contract-cleanup`** — Test-only wrappers, duplicate progress DTOs, orphan daemon methods, unused IPC commands, legacy listener helpers, and untested CLI modes form plausible but false implementation paths. _Raised by:_ coordination-survey, coordination-adversarial, wire-survey, wire-adversarial, daemon-survey.
- **`test-global-context-scanner-isolation`** — Process-global overrides and environment variables are guarded by non-cooperating locks, some restore only on normal fall-through, and the end-to-end scanner harness still reads the host tmux pane inventory. _Raised by:_ coordination-adversarial, harness-survey, harness-adversarial, tests-procedures-survey, tests-procedures-adversarial.
- **`daemon-worker-raii-recovery`** — A dead watch-owner sender is reused silently, WatchRuntime leaks subscriptions on early return, usage in_flight can stick forever after panic, and several long-lived loops/latches have no app-scoped cancellation or panic-safe reset. _Raised by:_ daemon-survey, daemon-adversarial.
- **`test-module-move-only-split`** — Four external test modules total 15,676 lines and 309 tests, which harms selection and parallel review, but one adversarial lens warns that moving them immediately before W4 risks the coverage that currently protects coordination. _Raised by:_ coordination-survey, tests-procedures-survey, tests-procedures-adversarial.
- **`harness-session-identity-classification`** — macOS cannot verify Claude procStart, Claude/Codex declare no non-session argv exclusions, transcript-path tool inference ignores configured homes, and same-worktree compaction resolution can collapse after session identity is lost. _Raised by:_ coordination-adversarial, harness-adversarial, wire-adversarial.
- **`authoritative-account-resolution`** — Settings, the account store, AccountChip, AccountChooser, and MeshTeamBuilder each re-order account candidates or parse selectors, and launch versus preview build different resolver inputs. _Raised by:_ harness-survey, harness-adversarial, frontend-survey, frontend-adversarial.
- **`method-typed-daemon-rpc`** — Work-doing RPCs inherit the 5-second ping timeout, ambiguous launch timeout falls back to a second local launch, one error can drop the pool, response IDs are unchecked, refresh_usage ignores its payload, and error codes are re-derived from prose. _Raised by:_ harness-survey, harness-adversarial, wire-survey, wire-adversarial.
- **`harness-registry-conformance`** — The central registry is surrounded by parallel descriptor, provider-root, fallback-roster, version, and binding truths; conformance tests miss important capability implications and launch content. _Raised by:_ harness-survey, harness-adversarial, frontend-survey.
- **`daemon-server-single-listener-runtime`** — Test helpers and the daemon binary reserve a port by bind/drop/rebind, test serving initializes a global scanning hub before the real bind, and per-server handler/counter state is not fully owned or joined. _Raised by:_ harness-adversarial, daemon-survey, daemon-adversarial, tests-procedures-survey, tests-procedures-adversarial.
- **`frontend-theme-contract-guard`** — Production contains a large existing set of inline dark-mode color ternaries despite the named-derived-token rule, and eight muted tokens use identical light/dark zinc-500 branches below AA on the dark panel. _Raised by:_ frontend-survey, frontend-adversarial.
- **`popup-keyboard-focus-contract`** — AccountChooser can let bubbled Enter choose a different account than the focused row and lacks modal containment; AccountChip lacks menu navigation; ContextMenu visual focus is not exposed to assistive technology; RoleCatalog popups have no menu [...] _Raised by:_ frontend-survey, frontend-adversarial.
- **`harness-module-decomposition`** — models/mod.rs and accounts/mod.rs mix unrelated domain, settings, resolver, observation, transport, and provider contracts in high-churn files. _Raised by:_ harness-survey.
- **`workflow-lib-drift-lint`** — The five .claude/workflows/*.js scripts carry a byte-identical shared lib by convention (scripts cannot import); nothing enforces equality, so a fix to one procedure's fail-closed logic can silently miss the others. _Raised by:_ tests-procedures-survey-r2.
- **`frontend-state-lifecycle-cleanup`** — Stopping polling without flush credits unobserved hidden time as activity, project switch leaves the resume tray alive across projects, and unwatching workflow sessions retains their full entries forever. _Raised by:_ frontend-adversarial.

Raw material: the twelve research reports, `candidates.json`, `candidates-judged.json` and `ranked.json` live in the orchestrating session's scratchpad (`debt-map/`); the judges' full notes per candidate are in `ranked.json`.
