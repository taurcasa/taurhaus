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
| #950 | dev-3 | completed | EPIC G3: move delivery, read/ack capture, nudges, escalation, and runtime automation onto canonical workflow state |
| #951 | mesh-architect | completed | EPIC G4: implement the reusable external adapter boundary plus the Claude Code task-file adapter |
| #952 | architect-1 | completed | EPIC G5: cut over CLI and operator flows to the greenfield core and remove touched legacy workflow assumptions |
| #953 | architect-1 | completed | Define the cross-epic acceptance scenario matrix and no-hidden-fallback review framework for the Mesh greenfield rollout |
| #954 | architect-1 | completed | Review the landed G3/G4/G5 slices against the acceptance matrix and identify immediate contract divergence, missing seams, and no-hidden-fallback risks |
| #955 | dev-3 | completed | Build executable greenfield acceptance verification scaffolding from the acceptance matrix |
| #956 | mesh-architect | completed | Add adapter conformance coverage for provenance, replay guards, degraded capabilities, conflict surfacing, and Claude compatibility boundaries |
| #957 | architect-1 | completed | Tighten the lifecycle event and delivery semantics contract for G1 and G3 based on the confirmed alignment-review divergences |
| #958 | dev-3 | completed | Implement the immediate runtime-side tightening from the alignment review, including prompt cleanup and stronger workflow-native delivery behavior |
| #959 | mesh-architect | in_progress | Wire external adapter ingress into a live Mesh operating path using the landed G1/G4 seams |
| #960 | dev-2 | in_progress | Wire projection-backed read surfaces into live Mesh paths without reintroducing snapshot archaeology |
| #961 | dev-3 | in_progress | Extend the acceptance harness for the next projection- and attention-driven scenarios unlocked by the landed slices |
| #962 | architect-1 | in_progress | Define the remaining G1/G2 contract for assignment supersession and task-created payload completeness |

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
- `#952` is complete. `architect-1` landed commit `e8942b7` in `/home/mstie/projects/mesh`, added explicit operator workflow commands for assign/accept/start/progress/block/review/complete, removed the synthetic `read --unread` task-resume fallback, updated `USAGE.md`, and documented cutover/deletion status in `docs/analysis/g5-cutover-deletion-ledger-2026-03-11.md`. Residual first-class assignment IDs, dedicated recovery/attention projections, and workflow-native delivery/read/ack remain dependent on G1-G4.
- `#953` is complete. `architect-1` wrote `/home/mstie/projects/mesh/docs/analysis/greenfield-acceptance-matrix-2026-03-11.md` and committed it as `3088427`. The document gives G1-G4 a direct epic-close framework across assignment lifecycle, recovery/resume, delivery/read/ack, progress/blocked/review/complete, attention/escalation, adapter degraded-capability, and cutover/no-hidden-fallback scenarios, with explicit minimum evidence and cross-epic close-review criteria.
- `#951` is complete. `mesh-architect` landed the G4 adapter work in `/home/mstie/projects/mesh` across commits `9e7d2c0c7cf3840056df4f70c5b004c8332f3f1a` and `0f6a7edd1815f75ce0ae89c0a1f5727abf60a7c1`, covering the reusable `ExternalWorkflowAdapter` boundary, capability model, external object mapping registry, provenance/replay guards, degraded-capability and conflict handling, and Claude Code ingress/egress/watch paths. The remaining dependency from G4 into G1 is the external observation event vocabulary / command sink needed to feed the canonical workflow core directly.
- `#948` is still active, but `dev-1` landed a major checkpoint in `/home/mstie/projects/mesh` as commit `8ac2643`. That slice adds explicit workflow lifecycle events and replay state for assign/accept/start/progress/block/review/complete/close in `src/workflow.rs`, stable `assignment_id` emission in `src/main.rs`, canonical-assignment-driven daemon wake logic in `src/daemon.rs`, the adapter/core ingest seam in `src/external_workflow/mod.rs`, exact snapshot replacement support in `src/tasks.rs`, and registry path helpers in `src/paths.rs`. Reported remaining blockers in the same contract lane are: no explicit close CLI path yet, delivery outcome still keyed to assignment message ids instead of first-class assignment ids, and the external adapter ingress seam is still a library/core boundary rather than a live daemon or CLI operating path.
- `#949` is still active, and `dev-2` landed a compile-baseline checkpoint in `/home/mstie/projects/mesh` as commit `8ae81fd`. That slice restores the missing projections module by landing `src/projections.rs`, projection-path support in `src/paths.rs`, and the `src/lib.rs` re-export so crate compilation can proceed again. Reported remaining G2-side contract caveats are that the current workflow vocabulary still lacks explicit `assignment_superseded` semantics and task-created workflow facts still do not carry full task title/description, so projection code must preserve those fields as unknown instead of inventing them.
- `#950` is complete. `dev-3` landed commit `86db1a5` in `/home/mstie/projects/mesh`, adding canonical workflow journal replay plus CLI and idle-monitor wiring for task-state, send, read, ack, nudge, escalation, and workflow-backed ack-status. Reported validation: `cargo test --quiet` passed. The only caveat is that the workflow event shape was inferred from current code because G1/G2 had not yet confirmed the contract explicitly.
- `#954` is complete. `architect-1` wrote `/home/mstie/projects/mesh/docs/analysis/greenfield-slice-alignment-review-2026-03-11.md` and committed it as `8e59c7b`. The review found confirmed divergence in four areas: coarse workflow journal vocabulary versus the lifecycle semantics exposed by G5, assignment wake-up still driven by owner-mutation detection instead of canonical assignment events, daemon assignment notifications still teaching the fenced legacy completion path, and missing canonical delivery outcome/retry semantics. It also separated these from acceptable staged incompleteness such as missing dedicated recovery/attention projections, first-class assignment IDs, and the still-unwired G4 adapter/core seam.
- `#955` is complete. `dev-3` landed commit `e8fe600` in `/home/mstie/projects/mesh`, adding a reusable `AcceptanceHarness` plus matrix-driven scenarios in `tests/acceptance_matrix.rs`. Covered representative scenarios include A5/A6 explicit accept+start lifecycle, C1/C2/C3 delivery-read-ack workflow linkage, D2 blocked durability, E2 blocked-task nudge suppression, and B3 workflow replay of current attention state. The harness also exposed and fixed a real issue: the idle-monitor raw fallback now excludes blocked tasks. Remaining harness gaps are explicitly listed for future slices.
- `#956` is complete. `mesh-architect` landed commit `c5464901ac9c4bf6453a8fbe0d7238aeed54d053` in `/home/mstie/projects/mesh`, adding direct adapter conformance coverage for provenance persistence and replay-guard rollover, degraded capability visibility, semantic-loss and conflict surfacing, delete observations with and without mappings, malformed JSON typed-error handling without hidden fallback, and canonical workflow journal retention of degraded/conflict metadata for external observations. Reported verification: `cargo test --lib external_workflow` passed (`17` tests) and `cargo test --lib` passed (`262` tests).
- `#957` is complete. `architect-1` wrote `/home/mstie/projects/mesh/docs/analysis/lifecycle-delivery-contract-tightening-2026-03-11.md` and committed it as `caae4ed`. The contract now fixes the intended greenfield semantics for explicit lifecycle events, delivery attempt/success/failure/retry/superseded semantics, assignment-native wake-up truth, explicit lifecycle operator wording, minimum assignment-id requirements, and the tightening order between G1 and G3.
- `#958` is complete. `dev-3` landed commit `5565f48` in `/home/mstie/projects/mesh`, removing the legacy completion prompt regression from daemon assignment notifications, teaching the explicit lifecycle path instead, and tightening runtime assignment delivery to prefer workflow task assignment state plus linked attention/message delivery state over legacy owner-mutation inference. Reported passing tests include `workflow_assignment_detection_prefers_task_state_and_assignment_delivery_link`, `task_notification_basic`, and `acceptance_matrix`. The remaining explicit dependency on G1 is that delivery outcome still attaches to assignment message ids rather than first-class assignment ids, so runtime dedupe still leans on `attention.last_assignment_message_id` until the core vocabulary grows assignment-level delivery identity.
- `#959` is active. `mesh-architect` is moving the adapter layer from contract-plus-conformance into a real live operating seam by wiring external observations into at least one Mesh entrypoint.
- `#960` is active. `dev-2` is moving the persisted projections from query-capable storage into actual live consumer paths while keeping unknown fields explicit.
- `#961` is active. `dev-3` is expanding the executable acceptance harness into the next projection- and attention-driven scenarios now unlocked by the landed slices.
- `#962` is active. `architect-1` is defining the remaining G1/G2 contract for assignment supersession and task-created payload completeness so core and projection lanes stop guessing independently.
