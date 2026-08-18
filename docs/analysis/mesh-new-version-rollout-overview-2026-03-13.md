# Mesh New-Version Rollout Overview - 2026-03-13

This document is the one-file reference for the current new-Mesh rollout state. It is meant to be referenced from other projects so they can understand what changed, what is already live, and which docs to read next.

## Current rollout status

- The new Mesh line is live in daily use.
- Taurhaus is pinned to Mesh `0.2.12`.
- The installed Windows Taurhaus build now bundles Mesh commit `fabb518681d6f4336e715ae2a22ed2f3166b4db9`.
- Live Mesh daemon alignment across the checked running teams is clean.

## What changed in the new Mesh

### 1. New workflow model

- The new Mesh is built around explicit workflow events, lifecycle rules, projection-backed views, and task-aware runtime behavior.
- Human-readable architecture comparison:
  - `/home/user/projects/mesh/docs/architecture/mesh-architecture-overview-old-vs-new.md`

### 2. New task lifecycle usage

- Operators should use explicit lifecycle commands instead of treating `task update` as the normal workflow path.
- The main operator guide is:
  - `/home/user/projects/mesh/docs/analysis/mesh-cli-migration-guide-for-operators-2026-03-12.md`

### 3. Action-first task and reminder wording

- Assignment, nudge, and resume prompts were changed so agents are told to start work now and only reply after real progress, a blocker, or completion.
- Key references:
  - `/home/user/projects/mesh/docs/analysis/mesh-assignment-wording-action-first-2026-03-12.md`
  - `/home/user/projects/mesh/docs/analysis/mesh-nudge-resume-wording-action-first-2026-03-12.md`
  - `/home/user/projects/mesh/docs/analysis/mesh-task-assignment-wording-drift-2026-03-12.md`

### 4. Team-lead admin task repair controls

- `team-lead` can now explicitly repair task state for other owners in specific cases.
- Supported lead-admin repair actions are documented here:
  - `/home/user/projects/mesh/docs/analysis/mesh-team-lead-task-admin-controls-2026-03-13.md`
- Implementation details are here:
  - `/home/user/projects/mesh/docs/analysis/mesh-team-lead-task-admin-controls-implementation-2026-03-13.md`
- Operator-facing usage notes are here:
  - `/home/user/projects/mesh/docs/analysis/mesh-team-lead-task-admin-operator-guide-2026-03-13.md`

### 5. Machine-safe task creation

- `mesh task create` now supports machine-safe output for orchestration.
- Default output is richer and includes the created subject.
- Structured output is documented here:
  - `/home/user/projects/mesh/docs/analysis/mesh-machine-safe-task-create-implementation-2026-03-13.md`
- Operator-facing notes are here:
  - `/home/user/projects/mesh/docs/analysis/mesh-machine-safe-task-create-operator-guide-2026-03-13.md`

### 6. Real broadcast command

- Mesh now has a team broadcast command instead of requiring manual send loops.
- Reference:
  - `/home/user/projects/mesh/docs/analysis/mesh-team-broadcast-command-2026-03-13.md`

### 7. Claude task-file metadata preservation

- Mesh no longer drops important task metadata when Claude task files are re-ingested.
- Reference:
  - `/home/user/projects/mesh/docs/analysis/mesh-claude-task-metadata-preservation-2026-03-12.md`

### 8. Communication recovery and stability

- The live communication failure was root-caused and fixed.
- Current communication state after recovery:
  - `/home/user/projects/mesh/docs/analysis/mesh-communication-state-after-recovery-2026-03-12.md`

## Taurhaus-side rollout and stability work

### 1. Windows rollout

- Taurhaus was repinned and rolled forward to the new Mesh build.
- Rollout report:
  - `/home/user/projects/taurhaus/docs/analysis/mesh-windows-rollout-2026-03-13.md`
- Windows install verification:
  - `/home/user/projects/taurhaus/docs/analysis/mesh-windows-install-verification-2026-03-13.md`
- Windows stale-artifact root cause and fix:
  - `/home/user/projects/taurhaus/docs/analysis/mesh-windows-artifact-refresh-fix-2026-03-13.md`

### 2. Startup freeze fix

- The post-install startup freeze was investigated and fixed at root cause.
- Report:
  - `/home/user/projects/taurhaus/docs/analysis/taurhaus-startup-freeze-investigation-2026-03-13.md`

### 3. Daemon alignment

- Live Mesh member and team daemons were checked across running teams and found aligned with the installed Mesh binary.
- Report:
  - `/home/user/projects/taurhaus/docs/analysis/mesh-live-daemon-version-alignment-2026-03-13.md`

## Team-facing documentation already updated

- Taurhaus team instructions now include current Mesh usage:
  - `/home/user/projects/taurhaus/AGENTS.md`
- Taurhaus-side update note:
  - `/home/user/projects/taurhaus/docs/analysis/taurhaus-mesh-usage-docs-update-2026-03-13.md`
- Mesh capability docs update summary:
  - `/home/user/projects/taurhaus/docs/analysis/mesh-capabilities-docs-update-2026-03-13.md`

## Recommended rollout pattern for other projects

For another project that should start using the new Mesh capabilities:

1. Start with this overview document.
2. Read the operator migration guide:
   - `/home/user/projects/mesh/docs/analysis/mesh-cli-migration-guide-for-operators-2026-03-12.md`
3. Reread project-local team instructions:
   - especially `/home/user/projects/taurhaus/AGENTS.md` in this repo, or the equivalent team instructions in the other repo
4. Use the new lifecycle command model and stop relying on legacy `task update` for normal work.
5. Use the new team-lead repair controls and machine-safe task creation output where appropriate.

## Short practical summary

If someone asks “what is new?” the short answer is:

- new workflow/event model
- explicit lifecycle command flow
- action-first assignment and reminder wording
- team-lead admin repair controls
- machine-safe task creation output
- real team broadcast command
- Claude metadata preservation on re-ingest
- current Taurhaus/Windows rollout and startup fixes are already in place
