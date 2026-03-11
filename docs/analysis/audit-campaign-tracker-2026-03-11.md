# Work Tracker - 2026-03-11

This document tracks the current Taurhaus audit campaign, active investigation work, and the follow-up workflow for confirmed findings.

## Active audit tasks

| Task | Owner | Status | Framework | Deliverable |
| --- | --- | --- | --- | --- |
| #927 Security audit: full Taurhaus review via TowerSec | security-auditor | completed | TowerSec | `docs/analysis/security-audit-towersec-2026-03-11.md` |
| #926 Code quality audit: full Taurhaus review via Towercraft | code-quality-auditor | completed | Towercraft | `docs/analysis/code-quality-audit-towercraft-2026-03-11.md` |

## Additional active investigations

| Task | Owner | Status | Deliverable |
| --- | --- | --- | --- |
| #928 Investigate Windows app stalls and grey title-bar freezes | architect-1 | completed | `docs/analysis/windows-app-stall-investigation-2026-03-11.md` |

## Tabled investigations

| Task | Status | Current decision |
| --- | --- | --- |
| #928 | tabled after investigation | Revisit only if the Windows stall / grey-titlebar freeze recurs multiple times |

## Review outcome

### Confirmed actionable findings

- TowerSec F-02: global libgit2 owner-validation disable
- TowerSec F-03: plaintext HTTP external URL opening
- TowerSec F-04: macOS AppleScript interpolation of tmux session names
- Towercraft Q-REP-04: missing checked-in CI plus incomplete frontend structural gates
- Towercraft Q-AI-01: Mesh frontend controller is a single high-churn god controller
- Towercraft Q-AI-02: backend coordination behavior is still effectively file-wide
- Towercraft Q-AI-03: payload normalization is duplicated across UI code instead of centralized at the boundary
- Towercraft Q-PRD-05: startup composition root lacks behavioral coverage for critical branches

### Triage noise or deferred validation

- TowerSec F-01: user-directed accepted risk for Taurhaus use case; no remediation work should continue unless product direction changes
- TowerSec V-01: `osv-scanner` advisory output without confirmed Taurhaus exploit path
- TowerSec V-02: daemon auth token write-then-chmod race was not confirmed as a shipped default-risk finding
- Towercraft false-positive / non-finding items:
  - `cargo machete` flag on `tauri-build`
  - missing `.github/workflows` made `actionlint` not applicable
  - no checked-in Towercraft semgrep ruleset was available for reproducible execution

## Review and follow-up policy

After both audit reports are delivered:

1. Team lead reviews each finding for evidence quality, reproducibility, and whether it is a real issue or audit noise.
2. Confirmed critical and high findings always become remediation tasks, regardless of implementation effort.
3. Confirmed medium and low findings become remediation tasks when the implementation effort is reasonable.
4. Likely noise, weakly evidenced issues, and out-of-scope suggestions stay documented but do not enter the implementation backlog until validated.
5. Remediation tasks are then assigned across `dev-1`, `dev-2`, `dev-3`, and `architect-1` based on subsystem ownership and required depth.

## Remediation backlog

| Task | Owner | Status | Source finding(s) | Goal |
| --- | --- | --- | --- | --- |
| #930 | architect-1 | completed | TowerSec F-02 | Replace global libgit2 owner-validation disable with scoped WSL trust handling |
| #931 | dev-2 | completed | TowerSec F-03, F-04 | Restrict HTTP external opens and escape macOS tmux AppleScript interpolation |
| #932 | dev-3 | completed | Towercraft Q-REP-04 | Add checked-in CI and structural frontend quality checks |
| #933 | dev-2 | completed | Towercraft Q-AI-01, Q-AI-03 | Split Mesh controller boundaries and centralize payload normalization |
| #934 | architect-1 | completed | Towercraft Q-AI-02 | Split coordination transport/runtime by real change boundary |
| #935 | dev-3 | completed | Towercraft Q-PRD-05 | Add startup behavior coverage for critical success/deferred/failure paths |

## Commit-readiness gate

| Task | Owner | Status | Goal |
| --- | --- | --- | --- |
| #936 | dev-3 | completed | Run `just check`, fix all surfaced issues at root cause, and report a clean logical commit split |
| #937 | dev-3 | completed | Investigate how to speed up `just check` safely, with recommendations for modest parallelism (for example 4-8 cores) without reintroducing resource thrash |

## Additional active reviews

| Task | Owner | Status | Goal |
| --- | --- | --- | --- |
| #938 | architect-1 | completed | Review Mesh task-management capabilities end to end, including Mesh source code, Taurhaus integration, concrete improvement ideas, task-domain architecture, and task-to-automated-message relationships from an AI-team user-story perspective |
| #940 | architect-1 | completed | Draft a handoff-ready Mesh communication/task-management architecture proposal with data model, user flows, gap closure plan, and architecture chart for the Mesh implementation team |
| #943 | mesh-architect | completed | Review the existing Mesh communication/task draft from native Mesh perspective, assess its viability against `/home/mstie/projects/mesh`, and produce a Mesh-first architecture proposal or counterproposal |
| #944 | mesh-architect + architect-1 | completed | Collaboratively produce a single final Mesh task/communication architecture proposal, reconcile the Taurhaus-side and Mesh-native perspectives, and require explicit approval from both architects |
| #945 | mesh-architect + architect-1 | completed | Jointly compare the approved linked-journal Mesh architecture against the earlier storage-heavy task-centric model, list pros and cons for both, and provide a final tradeoff recommendation with explicit sign-off from both architects |
| #946 | mesh-architect + architect-1 | completed | Jointly design the best greenfield Mesh architecture from Taurhaus and AI-team functional requirements first, focusing on fitness for purpose and ease of use rather than current implementation constraints |
| #947 | mesh-architect + architect-1 | completed | Extend the greenfield Mesh architecture with a Claude Code disk-file adapter proposal that preserves native Claude Code file interoperability where reasonable and explores a reusable plug-and-play adapter layer for future CLI tools |

## Greenfield implementation epics

| Task | Owner | Status | Goal |
| --- | --- | --- | --- |
| #948 | dev-1 | in_progress | EPIC G1: implement the canonical workflow core, command/event backbone, assignment lifecycle, and replay path in `/home/mstie/projects/mesh` |
| #949 | dev-2 | in_progress | EPIC G2: implement the projection layer and workflow views for lead, inbox, recovery, attention, and Taurhaus-facing surfaces |
| #950 | dev-3 | in_progress | EPIC G3: move delivery, read/ack capture, nudges, escalation, and runtime automation onto canonical workflow state |
| #951 | mesh-architect | in_progress | EPIC G4: implement the reusable external adapter boundary plus the Claude Code task-file adapter |
| #952 | architect-1 | in_progress | EPIC G5: cut over CLI and operator flows to the greenfield core and remove touched legacy workflow assumptions |

## Stopped backlog items

| Task | Status | Reason |
| --- | --- | --- |
| #929 | pending, unowned | User-directed accepted-risk decision; implementation work stopped immediately |

## Current state

- Both audit reports are complete and have been triaged into actionable findings versus audit noise.
- The urgent Windows stall investigation is complete; the report points to WSL-side daemon session scanning as the primary hotspot, with task refresh and Mesh polling as secondary amplifiers.
- The scoped libgit2 owner-validation remediation is complete; local libgit2 now rejects WSL UNC repos by default and the WSL exception path is narrowed to daemon-backed handling plus explicit filesystem marker validation where needed.
- The HTTP / AppleScript hardening task is complete. Plaintext `http://` external links are now blocked, opener capability is restricted to `https://**` plus `mailto:*`, and macOS tmux session names are escaped before AppleScript interpolation.
- The checked-in CI / structural-gate work is complete. One verification caveat remains outside that task's scope: `just test-fast` still hits 5 pre-existing `src/lib/ipc.test.js` normalization failures, which may overlap with task `#933`.
- The Mesh controller / normalization refactor is complete for the Mesh path. `dev-2` reported residual snake_case fallback risk in other non-Mesh template/UI surfaces, so that area may need a later follow-up if the remaining surfaces still map to the original audit concern after review.
- The backend coordination split is complete. `architect-1` reduced `commands/coordination.rs` to a thinner transport/orchestration layer and extracted `request_normalization.rs` plus `live_status.rs`, while noting `coordination/runtime.rs` and `coordination/orchestrator.rs` remain large follow-up hotspots.
- Startup behavior coverage for the critical success/deferred/failure branches is complete. `dev-3` noted an unrelated pre-existing clippy failure in `src/commands/coordination.rs`, which overlaps with the coordination area rather than the startup test work itself.
- Live monitor capture is already available at `/tmp/taurhaus-resource-monitor-v2.csv`.
- `#928` is tabled unless the Windows stall issue repeats often enough to justify a focused follow-up.
- `#936` is complete with a full green `just check` and a recommended logical commit split from `dev-3`.
- `#937` is complete. The main conclusion is that we should not chase speed by spawning many concurrent cargo jobs; the safer direction is two top-level lanes only (one cargo-driven Rust lane plus one frontend/Bun lane), with the first implementation step being removal of duplicated Rust test execution in `just check`.
- `#938` is complete. The main conclusion is that native Mesh already has a real AI-team task system, but its task domain is fragmented across snapshots, mutation journal, inbox/ack state, protocol index, and runtime side files; the recommended direction is a first-class workflow model built around `TaskSnapshot`, `TaskAssignment`, canonical `TaskEvent`, `TaskMessageLink`, `TaskRecoveryContext`, and `TaskAttentionState`.
- `#939` is complete. `just check` now uses the safe two-lane structure, Rust integration avoids replaying lib tests, and the measured full-gate wall-clock time dropped from about `320s` to `231.82s` without reintroducing concurrent cargo contention.
- `#940` is complete. The draft turns `#938` into a handoff-ready Mesh architecture proposal centered on a unified workflow system, authoritative workflow events plus projections, and first-class assignment/ack/progress/attention/recovery semantics.
- `#941` is complete. Root cause was stale first-match project-team inference in backend snapshot discovery combined with stale frontend project snapshot/cache reuse after initialize/runtime mutations. The chosen fix makes persisted project->active-team mapping the canonical source of truth and forces runtime flows to rehydrate from the canonical project snapshot after initialize/add/remove/resume.
- `#943` is complete. `mesh-architect` concluded that the goals of the original draft are sound but the proposed storage-heavy entity model is a poor fit for native Mesh; the recommended direction is linked journals plus lightweight projections.
- `#944` is complete. `mesh-architect` and `architect-1` converged on a joint proposal: keep explicit workflow semantics, implement them via enriched `task_mutations.jsonl` and `protocol_index.jsonl`, retain task snapshots and inboxes as operational surfaces, and add only two new per-task projections for recovery and attention. Both architects explicitly approved the final document.
- `#945` is complete. `mesh-architect` and `architect-1` delivered a joint tradeoff analysis of the linked-journal design versus the earlier storage-heavy task-centric approach and recommend the linked-journal model as the right near- and mid-term target, provided Mesh adds semantic workflow events in `task_mutations`, richer assignment/ack/delivery/completion linkage in `protocol_index`, real recovery and attention projections, and early internal `agent_id` linkage.
- `#946` is complete. The greenfield conclusion from `mesh-architect` and `architect-1` is that the best requirements-first Mesh architecture is workflow-core first: one authoritative append-only event backbone with purpose-built read models for lead workflow, agent inbox/action queue, recovery, attention, and Taurhaus integration. It is functionally closer to the storage-heavy task-centric direction than to the incremental linked-journal plan, but it is not the same as the earlier many-canonical-file proposal; it uses explicit domain objects over an event-backed core with projection-driven runtime views.
- `#947` is complete. `mesh-architect` and `architect-1` concluded that a Claude Code disk-file adapter is viable around the greenfield Mesh workflow-core architecture, but only as a capability-limited external ingress/egress adapter. Claude task files can support task snapshot interoperability, while richer workflow semantics such as assignment, acknowledgment, acceptance/start, structured progress, recovery, attention/escalation, and stable actor identity remain Mesh-native and require explicit degraded-capability handling in the adapter design.
- The greenfield execution roadmap is captured in `docs/analysis/mesh-greenfield-implementation-epics-2026-03-11.md`.
- `#948` through `#952` are now active as the new assignment units. We are treating them as epic containers rather than microtasks so owners can work longer without orchestration churn.
- Program rules for the greenfield rollout are now explicit: no legacy fallbacks in touched core paths, adapter-based compatibility at the edge only, preserve uncertainty instead of faking semantics, commit at stable checkpoints, and message team-lead before going idle when blocked or unclear.
