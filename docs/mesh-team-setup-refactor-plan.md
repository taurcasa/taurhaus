# Mesh Team Setup Refactor Plan

## Goal
Unify team onboarding/session launch architecture so mesh setup reuses the same launch/session-tracking path as context-menu launches, with deterministic behavior across Linux/macOS/Windows(WSL).

## Constraints
- Keep team files authoritative in WSL/Linux filesystem for mesh compatibility.
- Avoid test-only behavior leaking into production code paths.
- Preserve existing user-visible onboarding steps and progress events.

## Task 1: Lock Current Architecture Map
Status: completed

Description:
- Record current launch paths and mismatch points (context-menu vs coordination pipeline).

Acceptance criteria:
- Identified shared components and duplicated behavior boundaries.
- Identified config fields currently inconsistent with mesh expectations (`leadSessionId`, lead `agentType`, paths).

Tests:
- N/A (analysis-only).

Dependencies:
- None.

## Task 2: Shared Launcher API Extraction
Status: pending

Description:
- Extract a shared launcher service used by both context-menu flow and coordination flow.
- Centralize tmux availability checks, pane targeting, command assembly, and daemon launch fallback.

Acceptance criteria:
- Coordination no longer uses an isolated launch implementation for Claude/Codex/Gemini sessions.
- Context-menu and coordination call the same launcher entrypoint.

Tests:
- Unit tests for launcher command resolution and override validation.
- Unit tests for error mapping consistency across both callers.

Dependencies:
- Task 1.

## Task 3: Canonical Path Resolution for Coordination
Status: pending

Description:
- Resolve project input via the same project/provider path logic used by context-menu launches.
- Persist Linux/WSL canonical paths to mesh team config.

Acceptance criteria:
- Windows onboarding writes paths that mesh/CLI tools running in WSL can consume directly.
- UNC-only paths are not persisted to mesh team config when Linux path is available.

Tests:
- Unit tests for path normalization (`to_linux`) for Windows and non-Windows.
- Integration-style tests for config write payload path fields.

Dependencies:
- Task 2.

## Task 4: Lead Session Identity Sequencing
Status: pending

Description:
- Launch/attach lead session first, capture real lead session id, then persist lead metadata.
- Remove synthetic lead session id generation.

Acceptance criteria:
- `leadSessionId` in team config matches an actual launched lead session id.
- Lead member metadata aligns with mesh expectations for Claude lead behavior.

Tests:
- Store serialization tests for lead fields.
- Pipeline tests verifying write order: launch lead -> capture id -> persist config -> onboarding.

Dependencies:
- Tasks 2-3.

## Task 5: Onboarding Delivery Reliability
Status: pending

Description:
- Make onboarding prompt delivery resilient (pane readiness wait/retry bounded by timeout).
- Ensure first prompt is reliably entered for newly created panes.

Acceptance criteria:
- Prompt delivery does not produce empty newline-only injection in normal startup timing.
- Retries are bounded and observable in logs.

Tests:
- Runtime mock tests for delayed pane readiness and retry behavior.
- Failure test for timeout path and cleanup behavior.

Dependencies:
- Task 2.

## Task 6: End-to-End Linux Onboarding Test Harness
Status: pending

Description:
- Add Linux E2E-like orchestration test using fake backend + controlled runtime to validate full pipeline.
- Assert lead + member launch, mesh join, daemon startup, and onboarding sequence.

Acceptance criteria:
- One test covers happy-path onboarding sequence end-to-end.
- One test covers failure rollback without affecting unrelated panes/sessions.

Tests:
- Added under coordination pipeline/orchestrator test modules.

Dependencies:
- Tasks 2-5.

## Task 7: Windows/WSL Regression Coverage
Status: pending

Description:
- Add targeted regression tests for Windows host + WSL execution assumptions.

Acceptance criteria:
- Prevent regressions for `team not found`, wrong-path writes, and lead identity mismatch.

Tests:
- Scenario tests with Windows-style inputs and expected Linux-side outputs.

Dependencies:
- Tasks 3-6.

## Task 8: Testability Cleanup (Remove Scattered Test Flags)
Status: pending

Description:
- Replace ad-hoc test flags with trait-driven runtime/backends in coordination pipeline tests.

Acceptance criteria:
- No new production flags introduced solely for tests.
- Existing test seams are explicit (mock runtime/backend/stores) and localized.

Tests:
- Existing suite green with updated seams.

Dependencies:
- Tasks 2-7.

## Task 9: Validation + Commit Strategy
Status: pending

Description:
- Run focused test gates first, then broader `just test` once safe.
- Create logical split commits.

Acceptance criteria:
- Commits are organized by concern:
  1. shared launcher extraction,
  2. path/session identity fixes,
  3. onboarding reliability + tests,
  4. cleanup/refactor.

Tests:
- Required: targeted coordination tests + targeted session launch tests.
- Then: `just test`.

Dependencies:
- Tasks 2-8.

## Execution Order
1. Task 2
2. Task 3
3. Task 4
4. Task 5
5. Task 6
6. Task 7
7. Task 8
8. Task 9
