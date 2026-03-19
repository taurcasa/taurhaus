# Quality Phase Plan

Date: 2026-03-19
Owner: architect-1
Status: updated with user decisions, three-track execution model, and full task breakdown

## Scope and inputs

This plan is based on:

- `CLAUDE.md`
- `ARCHITECTURE.md`
- `README.md`
- `docs/README.md`
- `docs/docs-triage-report.md`
- `docs/operations/testing-guide.md`
- `e2e/README.md`
- current E2E specs in `e2e/specs/`
- key feature docs: project management, first run/settings, command center, session management, mesh
- supporting architecture audit notes in `docs/architecture/data-architecture.md`

The goal is to run a thorough quality phase without mixing current-state verification, code fixes, and documentation refresh into one noisy stream. Code and behavior should be corrected first; docs should then be updated to describe the corrected product honestly.

## Locked decisions from audit triage

These decisions are now fixed inputs to the phase plan:

- `F-01` unsafe launch flags: accepted risk. Do not change behavior in this phase; document the accepted risk clearly.
- `F-02` tmux API key exposure: accepted risk. Already reviewed and accepted; do not schedule remediation work in this phase.
- `F-03` `tantivy` / `lz4_flex`: fix only if it remains a clean dependency bump with low integration risk. Skip if it turns into a deeper compatibility task.
- `Q-PRD-01`: fix.
- `Q-PRD-02`: fix.
- `Q-PRD-03`: fix.
- Refactoring/generalization is now a first-class track, not a side effect of fixes.

## Execution model: three parallel tracks

The quality phase should run as three coordinated tracks:

1. Fix track
   - Mandatory product-honesty and correctness fixes.
   - Includes `Q-PRD-01/02/03`, opportunistic `F-03` if cheap, and any other confirmed UI/backend honesty gaps found during review.

2. Refactoring track
   - Oversized-file splits, duplication reduction, and targeted generalization that lowers future change risk.
   - Use `#1306` as additional input when it lands, but do not wait on it to start the already-known hotspot work.

3. Review track
   - E2E gap closure, functional-honesty review, error/empty-state review, accessibility, observability, and docs truth pass.
   - Documentation edits happen after the fix and refactor waves stabilize.

## Recommended review categories

Beyond security and code quality, the quality phase should explicitly cover these categories:

1. Functional honesty review
   - Ask of every user-facing control: if a user clicks it, does the backend really do what the UI implies?
   - Highest risk areas are settings, command center, session/runtime recovery, and mesh lifecycle states.

2. Workflow coverage review
   - Audit whether critical user journeys have either E2E coverage or strong integration/unit coverage.
   - Focus on project registration, session control, search, tasks, git navigation, templates, and mesh recovery.

3. Documentation accuracy review
   - Confirm active docs match shipped behavior, current commands, current file ownership, and current platform rules.
   - Defer edits until post-fix stabilization.

4. Error and empty-state quality review
   - Check validation copy, recovery guidance, degraded-state messaging, and whether operators get actionable next steps.
   - Prioritize first-run, daemon/connectivity, mesh preflight, and project registration failures.

5. Accessibility and keyboard-flow review
   - Verify keyboard navigation, focus return, dialog escape behavior, semantic roles, and visible state changes.
   - Current E2E only lightly covers shortcuts and close behavior.

6. Logging and observability completeness review
   - Confirm important failure paths emit structured events with correlation IDs and enough context to debug.
   - Prioritize daemon RPC, coordination lifecycle, startup, and project/session mutation paths.

7. Performance and responsiveness review
   - Validate "snappy" product requirements on startup, tab switching, project switching, search open/close, and mesh runtime refresh.
   - Treat responsiveness regressions as product defects, not polish.

8. Release/install/platform parity review
   - Verify docs and behavior line up across Linux dev, Windows release, and macOS build/test paths.
   - Highest risk areas are daemon install/update flows, WSL expectations, and mesh/tool prerequisites.

## Current E2E coverage snapshot

Current suites cover a good amount of happy-path surface area:

- overview interactions
- project lifecycle basics
- files workflow
- git workflow
- tasks workflow
- search workflow
- settings persistence
- theme and shortcut behavior
- context menu basics
- cross-tab navigation
- daemon-connected resilience smoke coverage
- mesh setup/runtime happy path
- templates CRUD basics
- regression guards
- screenshot lanes

This is a strong baseline for tab-level coverage, but it is not yet a complete workflow-level safety net.

## E2E gaps and under-tested workflows

### Tier A: highest-value missing coverage

1. First-run wizard end-to-end
   - No dedicated spec for welcome -> daemon setup -> browse -> selection -> progress -> completion.
   - This is the main onboarding path and a major documentation surface.

2. Command center real actions
   - Existing coverage confirms buttons exist, but not that continue/fresh/resume/stop/restart/open-terminal flows behave correctly.
   - This is the clearest functional-honesty risk in the current suite.

3. Session-management live behavior
   - No focused E2E for active/idle transitions, sidebar badge truth, hover-card runtime details, session history drill-down, or click-through navigation to active panes.
   - Current daemon integration spec is mostly a smoke check.

4. Mesh degraded and recovery paths
   - Happy-path init/hot-add/disband exists, but not cold resume, per-member resume, re-onboard, removal warnings, conflict recovery, or compaction audit visibility.

### Tier B: important but slightly lower risk

5. Project management completeness
   - Quick scan add flow, remove-from-taurhaus confirmation, relationship creation/removal, and overview relationship recovery are not strongly covered.

6. Template history and provenance
   - CRUD is covered, but history/diff/revert/import/export/provenance behavior is not clearly guarded.

7. Search freshness and indexing lifecycle
   - Search open/navigation is covered, and settings can rebuild index, but there is no workflow proving search results refresh correctly after content changes or rebuild recovery.

8. Error-path depth
   - Current error suite covers validation basics, but not mesh preflight failures, daemon install/update failures, startup bootstrap failure messaging, or cross-platform recovery guidance.

### Tier C: coverage that should exist before release hardening

9. Accessibility-focused keyboard travel
   - Focus order, dialog traps, context menu keyboard actions, and settings/tab traversal are not explicitly exercised.

10. Logging/diagnostic visibility
   - Not a classic E2E target, but milestone verification should include smoke assertions or scripted checks for expected logs on critical failures.

11. README/documented-user-journey parity
   - README screenshots exist, but there is no checklist ensuring README-promised workflows are all actually exercised by tests or manual review.

## Documentation review scope

Documentation work should happen after fixes land, but we can identify the likely stale set now.

### Highest-risk active docs

1. `ARCHITECTURE.md`
   - The file already self-identifies at least one stale infographic and is dense enough that drift is easy.
   - Needs a post-fix pass for command counts, watcher ownership, coordination/runtime details, and any audit-driven corrections.

2. `README.md`
   - High visibility and broad promises. Should be checked against actual command-center, mesh recovery, session context, and install behavior after the code settles.

3. `docs/README.md`
   - Likely mostly current, but should be verified after any archive movement or doc rewrites.

4. `docs/getting-started.md`
   - High risk because install, daemon, shell, and troubleshooting instructions drift quickly.

5. `docs/features/first-run-and-settings.md`
   - Contains a direct honesty note that scan directories / ignore patterns are persisted but not wired at runtime.
   - Must be rechecked once code-quality findings land, because this is exactly the kind of UI/backend mismatch we should either fix or label clearly.

6. `docs/features/command-center.md`
   - Needs verification against actual launch/resume/stop behavior and platform decision trees.

7. `docs/features/session-management.md`
   - Dense and implementation-heavy; needs a truth pass after session/runtime fixes.

8. `docs/features/mesh.md`
   - High drift risk because mesh architecture and UI have been changing rapidly.

9. `docs/coordination-architecture.md`
   - Already large and partly historical. Good candidate for split or trim once behavior stabilizes.

10. `docs/architecture/data-architecture.md`
   - New and useful, but should be reconciled with `ARCHITECTURE.md` and coordination docs so there is one clear source for live data ownership.

### Lower-risk but still worth checking in the doc phase

- `docs/operations/testing-guide.md`
- `docs/operations/build-and-release.md`
- `docs/platform-abstraction.md`
- `docs/team-templates.md`
- `docs/features/project-management.md`
- `docs/features/search.md`
- `docs/features/task-board.md`
- `docs/features/git-integration.md`
- `docs/file-rendering-pipeline.md`

### Existing triage input to reuse

`docs/docs-triage-report.md` is still useful as the baseline inventory. The new doc phase should not re-triage from zero; it should use that report and only refresh classifications that recent product changes invalidated.

## Recommended sequencing

The safest order is:

1. Lock the triage inputs and accepted-risk boundaries
   - Inputs: security `#1303`, code quality `#1304`, generalization audit `#1306`, and the user decisions above.
   - Output: one ranked backlog with three explicit buckets: fix, refactor, review.

2. Start fix and refactor tracks in parallel where write scopes do not collide
   - Fix mandatory honesty/correctness issues first.
   - Start low-conflict file-split/generalization work in hotspot files that are not being actively modified by fix tasks.

3. Add regression coverage alongside each fix
   - Unit/integration first for logic.
   - Expand E2E only on the highest-value workflow gaps.

4. Run review-track hardening after the core fixes are stable
   - Error/empty states, accessibility, observability, and manual product/design review.

5. Update docs last
   - Refresh high-traffic docs only after implementation and review findings settle.

6. Close with a release-readiness sweep
   - Re-run the prioritized test lanes and manual checks that best represent real operator use.

## Wave plan

### Wave 0: Triage lock and assignment setup

Goal:
- lock decisions, split the phase into three tracks, and assign the first safe parallel tasks

Deliverables:
- merged issue board grouped by fix / refactor / review
- explicit accepted-risk entries for `F-01` and `F-02`
- explicit "cheap only" gate for `F-03`
- dependency map for the execution waves below

### Wave 1: Fix track and low-conflict refactors in parallel

Goal:
- remove misleading behavior and reduce hotspot change risk without stepping on the same files unnecessarily

Priority targets:
- settings/runtime mismatches
- session/runtime state truth
- Linux terminal contract drift
- shared contract and helper extractions from the generalization audit
- oversized-file splits on known hotspots
- any cheap dependency hygiene work for `F-03`

Exit criteria:
- no known critical UI/backend mismatch remains unowned
- all mandatory fix tasks have matching regression coverage
- first refactor slices have landed without destabilizing the fix track

### Wave 2: Review-track expansion

Goal:
- close the highest-value workflow and review gaps against the stabilized implementation

Required additions:
- first-run wizard E2E
- command-center behavior E2E
- session-management/runtime truth E2E
- mesh degraded/resume E2E

Nice-to-have additions:
- template history/provenance
- project remove/relationship coverage
- search freshness after mutation/rebuild

Exit criteria:
- each top-tier workflow has at least one credible end-to-end path covered

### Wave 3: UX quality, accessibility, and observability

Goal:
- harden the operator experience around failure, recovery, and fast understanding

Targets:
- error copy and empty-state actionability
- keyboard/focus behavior
- log completeness on critical paths
- responsiveness regressions and loading-state honesty

Exit criteria:
- degraded and failure states are understandable without source diving
- major flows produce enough telemetry to debug field failures

### Wave 4: Documentation truth pass

Goal:
- make active docs match the shipped product exactly

Priority docs:
- `README.md`
- `ARCHITECTURE.md`
- `docs/getting-started.md`
- `docs/features/first-run-and-settings.md`
- `docs/features/command-center.md`
- `docs/features/session-management.md`
- `docs/features/mesh.md`
- `docs/coordination-architecture.md`

Exit criteria:
- high-traffic docs have no known stale promises
- docs index points to current sources and archived material cleanly

### Wave 5: Release-readiness gate

Goal:
- verify the quality phase actually improved ship confidence

Checks:
- selected E2E lane reruns
- manual product review on core workflows
- visual review on changed frontend surfaces
- platform/install sanity spot checks

Exit criteria:
- team-lead can make release or next-phase decisions from one concise status view

## Team allocation

Recommended staffing for a team of 3 devs + 1 architect + 1 designer + 1 product reviewer:

### Dev 1: backend/runtime correctness owner

Own:
- daemon/runtime/session truth issues
- command-center backend behavior
- persistence and recovery defects
- supporting Rust regression coverage

### Dev 2: frontend workflow and UX-hardening owner

Own:
- error states
- settings UX honesty
- accessibility and keyboard-flow fixes
- frontend regression/unit coverage

### Dev 3: integration and E2E owner

Own:
- highest-value E2E additions
- test harness reliability fixes
- workflow verification for first-run, command center, session runtime, and mesh recovery
- coordination with fix owners on reproducible acceptance checks

### Architect: triage and architecture guard

Own:
- merge findings across `#1303`, `#1304`, and this plan
- flag architectural theater
- prevent over-engineered fixes
- keep module boundaries and documentation ownership coherent
- gate doc updates on actual behavior stabilization

### Designer: failure-state and dense-UI clarity review

Own:
- review degraded, recovery, and empty states
- check visual hierarchy in settings, mesh runtime, and onboarding
- ensure fixes preserve "dense but calm" and "snappy" design principles

### Product reviewer: functional honesty and workflow acceptance

Own:
- validate that top workflows work as a user would expect
- verify that UI language matches actual backend behavior
- prioritize which gaps are ship blockers versus follow-up work

## Full task breakdown

Effort scale:

- `S` = less than half a day
- `M` = about half to one day
- `L` = one to two days
- `XL` = multi-day / should likely be sliced if it grows

| ID | Wave | Track | Task | Suggested owner | Effort | Depends on | Notes |
|---|---|---|---|---|---|---|---|
| `QPLAN-01` | 0 | Coordination | Merge `#1303`, `#1304`, this plan, and later `#1306` into one working backlog | `architect-1` | `S` | audit outputs | Converts audits into one execution board instead of parallel documents |
| `QPLAN-02` | 0 | Coordination | Record accepted-risk decisions for `F-01` and `F-02`, plus the cheap-only gate for `F-03` | `architect-1` | `S` | `QPLAN-01` | Update planning/risk docs so the team does not reopen settled debates |
| `QFIX-01` | 1 | Fix | Introduce a backend-owned scanner/index policy contract that reads saved `scan_directories` and `ignore_patterns` | `dev-1` | `L` | `QPLAN-01` | Core backend fix for `Q-PRD-01`; should avoid frontend-only workarounds |
| `QFIX-02` | 1 | Fix | Wire project discovery and index rebuild paths to the new scanner/index policy and add backend tests | `dev-1` | `M` | `QFIX-01` | Covers `scan_directory()` and search rebuild behavior |
| `QFIX-03` | 1 | Fix | Align Settings UI honesty with the scanner/index contract and add frontend tests for active behavior | `dev-2` | `M` | `QFIX-01` | If backend wiring slips, this task must at minimum prevent misleading active-state UI |
| `QFIX-04` | 1 | Fix | Fix session activity duration math to use the active poll interval and add exact-value tests | `dev-2` | `S` | `QPLAN-01` | Direct fix for `Q-PRD-02`; low-risk and should land early |
| `QFIX-05` | 1 | Fix | Centralize terminal emulator defaults/capabilities across Rust, frontend, and tests for Linux/macOS/Windows | `dev-2` | `M` | `QPLAN-01` | Direct fix for `Q-PRD-03` |
| `QFIX-06` | 1 | Fix | Review remaining functional-honesty mismatches surfaced during fix work and either fix or label them | `product-check-1` | `M` | `QFIX-01`,`QFIX-04`,`QFIX-05` | Product-check validates the visible truthfulness; implementation may spin follow-up tasks if needed |
| `QSEC-01` | 1 | Fix | Attempt `tantivy` / `lz4_flex` remediation only if it is a clean dependency bump with green audits/tests | `dev-1` | `S` | `QPLAN-02` | Skip immediately if upgrade causes compatibility churn |
| `QREF-01` | 1 | Refactor | Split [`src/Shell.svelte`](/home/mstie/projects/taurhaus/src/Shell.svelte) by orchestration concern boundaries | `dev-3` | `XL` | `QREF-05`,`QREF-07` | Highest frontend hotspot from residual risks; keep behavior identical and avoid starting before shared seams exist |
| `QREF-02` | 1 | Refactor | Split `src-tauri/src/startup/mod.rs` into smaller startup phase modules with thin composition root | `dev-1` | `L` | `QREF-05` | Schedule around `QFIX-01/02` to avoid file conflicts if startup paths overlap |
| `QREF-03` | 1 | Refactor | Slice `src-tauri/src/coordination/runtime.rs` into explicit runtime/state/service modules | `dev-3` | `XL` | `QREF-05`,`QREF-06`,`QREF-07` | Start only after confirming no conflicting active feature work touches the same module |
| `QREF-04` | 1 | Refactor | Reduce `src-tauri/src/coordination/stall_detector.rs` size by extracting diagnostics/decision helpers | `dev-1` | `L` | `QREF-05` | Lower urgency than the shared-seam extractions; can slide if fix track needs the same owner |
| `QREF-05` | 1 | Refactor | Fold `#1306` generalization findings into the refactor backlog, confirm overlap with `#1314`, and reorder refactor execution accordingly | `architect-1` | `S` | `#1306` | Generalization audit is advisory input; do not block known mandatory fixes on it |
| `QREF-06` | 1 | Refactor | Extract a shared tmux layout allocator used by coordination runtime and session scanning (`GR-05`) | `dev-1` | `M` | `QREF-05` | Create the shared helper before large coordination/runtime splits so layout policy stops drifting |
| `QREF-07` | 1 | Refactor | Consolidate project-path identity normalization into shared helpers per layer (`GR-07`) | `dev-1` | `S` | `QREF-05` | Small but high-leverage cleanup that should land before bigger orchestrator splits |
| `QREV-01` | 2 | Review | Add first-run wizard E2E coverage | `dev-3` | `M` | `QFIX-01`,`QFIX-02`,`QFIX-03` | Must verify the onboarding path described in docs actually works |
| `QREV-02` | 2 | Review | Add command-center real-action E2E coverage for continue/fresh/resume/stop/restart/open-terminal | `dev-3` | `L` | `QFIX-05` | Highest-value workflow gap after the mandatory fixes |
| `QREV-03` | 2 | Review | Add session-management runtime-truth E2E coverage for active/idle, hover details, history drill-down, and navigation | `dev-3` | `L` | `QFIX-04` | Focus on real behavior, not just badge presence |
| `QREV-04` | 2 | Review | Add mesh degraded/resume/re-onboard/removal-warning E2E coverage | `dev-3` | `L` | Wave 1 fixes stable | Can overlap with `QREV-01/02/03` if the harness remains stable |
| `QREV-05` | 2 | Review | Review and tighten error/empty-state messaging for onboarding, daemon, mesh preflight, and project registration | `product-check-1` | `M` | Wave 1 fixes stable | Product reviewer identifies the required copy/behavior changes |
| `QREV-06` | 2 | Review | Implement approved error/empty-state UX improvements | `dev-2` | `M` | `QREV-05` | Keep design language consistent with the existing shell |
| `QREV-07` | 2 | Review | Accessibility and keyboard-flow audit for dialogs, overlays, menus, settings, and tab travel | `product-check-1` | `M` | Wave 1 fixes stable | Outputs a focused punch list, not a generic audit memo |
| `QREV-08` | 2 | Review | Implement accessibility/keyboard-flow fixes and add targeted tests | `dev-2` | `M` | `QREV-07` | Prefer real keyboard flows over purely structural assertions |
| `QREV-09` | 3 | Review | Observability review for critical startup, daemon, project mutation, and coordination failure paths | `architect-1` | `M` | Wave 1 fixes stable | Identify missing structured events and debugging blind spots |
| `QREV-10` | 3 | Review | Implement agreed logging/diagnostic additions on critical paths | `dev-1` | `M` | `QREV-09` | Keep logging structured and correlation-friendly |
| `QDES-01` | 3 | Review | Visual and interaction review of onboarding, settings honesty, recovery/degraded states, and dense-panel clarity | `design-taurhaus` | `M` | Wave 1 and Wave 2 UI work stable | Designer should review changed states, not just happy paths |
| `QPROD-01` | 3 | Review | Functional-honesty pass across top workflows after fixes and E2E additions | `product-check-1` | `M` | `QREV-01`,`QREV-02`,`QREV-03`,`QREV-04` | Explicitly verify user-visible claims match backend behavior |
| `QDOC-01` | 4 | Review | Refresh `README.md`, `ARCHITECTURE.md`, and `docs/README.md` against shipped behavior | `architect-1` | `L` | `QPROD-01`,`QDES-01` | High-traffic docs first |
| `QDOC-02` | 4 | Review | Refresh `docs/getting-started.md` and active feature docs for first-run, settings, command center, session management, and mesh | `dev-2` | `L` | `QPROD-01`,`QDES-01` | Docs update happens only after behavior stabilizes |
| `QDOC-03` | 4 | Review | Reconcile architecture detail between coordination/data/platform docs and archive or trim stale overlapping material | `architect-1` | `M` | `QDOC-01` | Avoid conflicting "source of truth" explanations |
| `QGATE-01` | 5 | Coordination | Run milestone verification plan and summarize go/no-go confidence | `product-check-1` | `M` | `QDOC-01`,`QDOC-02`,`QDOC-03` | Include prioritized E2E reruns and manual checks |
| `QGATE-02` | 5 | Coordination | Produce the final quality-phase closure note with shipped fixes, accepted risks, residual gaps, and recommended next follow-ups | `architect-1` | `S` | `QGATE-01` | Final artifact for team-lead decision-making |

## Parallelism and conflict guidance

- `dev-1` should prioritize `QREF-07` and `QREF-06` before `QREF-02/QREF-04`, and should not run scanner/startup-heavy fix and refactor tasks in parallel if they converge on the same files.
- `dev-2` can run `QFIX-04` first, then `QFIX-05`, then move into `QREV-06/QREV-08/QDOC-02`.
- `dev-3` should not start the largest orchestrator splits until the shared seams from `QREF-06/QREF-07` are in place; after that, `QREF-01` is the best frontend hotspot to attack before pivoting into E2E expansion.
- `QREF-03` should wait if coordination runtime files are under active feature repair by another developer.
- `architect-1`, `design-taurhaus`, and `product-check-1` should feed decisions quickly so devs do not block on interpretation work.

## Critical path

The main critical path is:

`QPLAN-01` -> `QFIX-01` -> `QFIX-02` / `QFIX-03` -> `QREV-01` -> `QPROD-01` -> `QDOC-01` / `QDOC-02` -> `QGATE-01` -> `QGATE-02`

Secondary critical path:

`QPLAN-01` -> `QFIX-05` -> `QREV-02` -> `QPROD-01`

Refactor path:

`QPLAN-01` -> `QREF-05` -> `QREF-07` / `QREF-06` / `GR-06 overlap with #1314` -> `QREF-02` -> `QREF-01` / `QREF-03` / `QREF-04`

This refactor path is important, but it should yield to the fix path if the two start competing for the same files or verification bandwidth.

## Immediate next actions

1. Convert this task table into actual assignee work orders beginning with `QFIX-01`, `QFIX-04`, `QFIX-05`, and one safe refactor slice.
2. Record accepted-risk handling for `F-01` and `F-02` so no one spends cycle time reopening them.
3. Attempt `QSEC-01` only if the dependency bump stays clean.
4. Start `QREF-07` and `QREF-06` ahead of the largest file splits, since the generalization audit shows those shared seams should exist first.
5. Delay documentation edits until the Wave 1 and Wave 2 implementation tasks are stable.

## Decision rules

- If a UI control exists but backend behavior is incomplete, either fix it or label/hide it before docs polish.
- Prefer adding a small number of high-value E2E workflows over broad low-signal screenshot churn.
- Do not update docs to match temporary behavior if the team already intends to correct the code in the same phase.
- Treat degraded-state recovery, onboarding, and session/mesh truth as more important than lower-risk polish.
