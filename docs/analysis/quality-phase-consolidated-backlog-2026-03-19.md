# Quality Phase Consolidated Backlog

Date: 2026-03-19
Owner: architect-1
Source inputs:

- `docs/analysis/taurhaus-security-full-audit-2026-03-19.md`
- `docs/analysis/taurhaus-code-quality-full-audit-2026-03-19.md`
- `docs/analysis/taurhaus-generalization-reuse-redundancy-audit-2026-03-19.md`
- `docs/analysis/quality-phase-plan-2026-03-19.md`

This document is the single execution reference for the current quality phase. It merges audit findings and plan decisions into one ranked backlog, grouped by the three active tracks:

1. fix
2. refactor
3. review

## Locked decisions

- `F-01` unsafe launch flags: accepted risk. Keep tracked in risk docs, but no implementation task in this phase.
- `F-02` tmux API key exposure: accepted risk. Keep tracked in risk docs, but no implementation task in this phase.
- `F-03` `tantivy` / `lz4_flex`: opportunistic only. Execute only if it stays a clean dependency bump.
- `Q-PRD-01/02/03`: mandatory fixes.

## Severity model used here

- `P0` = must land in this phase before docs close-out
- `P1` = strong candidate for this phase; should land unless blocked by higher-priority churn
- `P2` = valuable but can slip if conflict or churn grows
- `Accepted risk` = tracked, documented, not implemented in this phase

## Fix track backlog

### P0

1. `Q-PRD-01` Settings-backed scan and ignore policy is not enforced at runtime
   - Source: code quality audit
   - Why it matters: active UI control is misleading today
   - Planned tasks: `QFIX-01`, `QFIX-02`, `QFIX-03`
   - Owners: `dev-1`, `dev-2`

2. `Q-PRD-02` Session activity persistence undercounts Tauri fallback polling
   - Source: code quality audit
   - Why it matters: persisted activity metrics are wrong by 10x in a real app path
   - Planned task: `QFIX-04`
   - Owner: `dev-2`

3. `Q-PRD-03` Linux terminal settings drift across frontend, backend, and tests
   - Source: code quality audit
   - Why it matters: platform contract is inconsistent and the tests reinforce the wrong behavior
   - Planned task: `QFIX-05`
   - Owner: `dev-2`

4. Functional honesty follow-through on any remaining visible mismatch discovered while fixing `Q-PRD-01/02/03`
   - Source: quality phase plan
   - Why it matters: this phase should leave no obvious “UI says yes, backend says no” gap in the touched surfaces
   - Planned task: `QFIX-06`
   - Owner: `product-check-1` plus the relevant dev owner

### P1

5. `F-03` Search index dependency issue (`lz4_flex` via `tantivy`) if and only if a clean bump exists
   - Source: security audit
   - Why it matters: low severity but reachable shipped dependency issue
   - Planned task: `QSEC-01`
   - Owner: `dev-1`
   - Gate: stop immediately if dependency upgrade grows beyond a clean bump

### Accepted risk

6. `F-01` Unsafe launch flags are the default
   - Source: security audit
   - Decision: accepted risk for this audience and phase
   - Follow-up: keep explicitly documented with revisit trigger

7. `F-02` API keys remain visible within the shared `taurhaus` tmux session boundary
   - Source: security audit
   - Decision: accepted risk for this phase
   - Follow-up: keep explicitly documented with revisit trigger

## Refactor track backlog

### P0

1. `GR-06` Centralize terminal and CLI launch contract across frontend and backend
   - Source: generalization audit plus `Q-PRD-03`
   - Why it matters: this is both a refactor and a correctness prerequisite
   - Planned tasks: `QFIX-05`, with overlap note against existing task `#1314`
   - Owners: `dev-2`, `dev-1`

2. `GR-07` Consolidate project-path identity normalization into one authority per layer
   - Source: generalization audit
   - Why it matters: prevents repeated drift in cache identity, command-center matching, and coordination path comparison
   - Planned task: `QREF-07`
   - Owner: `dev-1`

### P1

3. `GR-05` Extract one shared tmux layout allocator for coordination and session scanning
   - Source: generalization audit
   - Why it matters: duplicated layout policy is likely to drift again
   - Planned task: `QREF-06`
   - Owners: `dev-1`, `dev-3`

4. `GR-04` Split `startup/mod.rs` into telemetry, setup phases, and orchestration runners
   - Source: generalization audit plus code-quality residual risks
   - Planned task: `QREF-02`
   - Owner: `dev-1`

5. `GR-01` Split `src/Shell.svelte` into composition plus focused controllers
   - Source: generalization audit plus code-quality residual risks
   - Planned task: `QREF-01`
   - Owner: `dev-3`

### P2

6. `GR-02` Split `coordination/runtime.rs` by runtime role
   - Source: generalization audit plus code-quality residual risks
   - Planned task: `QREF-03`
   - Owner: `dev-3`
   - Risk: large write scope; do not compete with active runtime bug work

7. `GR-03` Split `stall_detector.rs` into service, signals, transition engine, and history
   - Source: generalization audit plus code-quality residual risks
   - Planned task: `QREF-04`
   - Owner: `dev-1`
   - Risk: large write scope; should follow smaller shared-seam extractions

## Review track backlog

### P0

1. First-run wizard E2E coverage
   - Source: quality phase plan audit
   - Planned task: `QREV-01`
   - Owner: `dev-3`

2. Command-center real-action E2E coverage
   - Source: quality phase plan audit
   - Planned task: `QREV-02`
   - Owner: `dev-3`

3. Session-management runtime-truth E2E coverage
   - Source: quality phase plan audit
   - Planned task: `QREV-03`
   - Owner: `dev-3`

4. Mesh degraded/resume/recovery E2E coverage
   - Source: quality phase plan audit
   - Planned task: `QREV-04`
   - Owner: `dev-3`

### P1

5. Error and empty-state review for onboarding, daemon, project registration, and mesh preflight
   - Source: quality phase plan
   - Planned tasks: `QREV-05`, `QREV-06`
   - Owners: `product-check-1`, `dev-2`

6. Accessibility and keyboard-flow review plus fixes
   - Source: quality phase plan
   - Planned tasks: `QREV-07`, `QREV-08`
   - Owners: `product-check-1`, `dev-2`

7. Observability completeness review on critical paths
   - Source: quality phase plan
   - Planned tasks: `QREV-09`, `QREV-10`
   - Owners: `architect-1`, `dev-1`

8. Functional-honesty review after fixes and E2E additions
   - Source: quality phase plan
   - Planned task: `QPROD-01`
   - Owner: `product-check-1`

### P2

9. Documentation truth pass across high-traffic docs
   - Source: quality phase plan
   - Planned tasks: `QDOC-01`, `QDOC-02`, `QDOC-03`
   - Owners: `architect-1`, `dev-2`
   - Constraint: only after fix and review waves stabilize

10. Release-readiness closure summary
   - Source: quality phase plan
   - Planned tasks: `QGATE-01`, `QGATE-02`
   - Owners: `product-check-1`, `architect-1`

## Ranked execution order across all tracks

1. `Q-PRD-01` enforce scanner/index settings truth
2. `Q-PRD-02` correct session activity persistence math
3. `Q-PRD-03` unify Linux terminal contract
4. `GR-06` centralize terminal/CLI launch contract
5. First-run wizard E2E
6. Command-center real-action E2E
7. Session-management runtime-truth E2E
8. Mesh degraded/resume E2E
9. `GR-07` path normalization reuse cleanup
10. `GR-05` shared tmux layout allocator
11. `GR-04` startup split
12. `GR-01` shell split
13. `GR-02` coordination runtime split
14. `GR-03` stall detector split
15. Docs truth pass
16. Release-readiness close-out

## Parallel execution guidance

- Safe early parallel set:
  - `Q-PRD-01` backend contract wiring
  - `Q-PRD-02` activity-math fix
  - `Q-PRD-03` platform contract cleanup
  - one low-conflict refactor slice, preferably `QREF-07` or `QREF-06` before large file splits

- Review work should begin once the corresponding implementation surface stabilizes enough that tests are not chasing moving targets.

- Large orchestrator splits should not start until the shared contracts and helper seams they depend on are already in place.
- The generalization audit explicitly recommends: contracts first, shared helpers second, oversized-file splits third.

## Relationship to the master plan

This backlog is the compact execution view.

For owners, dependencies, effort sizing, and wave structure, see:

- `docs/analysis/quality-phase-plan-2026-03-19.md`
