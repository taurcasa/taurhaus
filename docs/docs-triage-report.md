# Docs triage report

Generated: 2026-03-07

Scope reviewed:
- every prose/config file under `docs/` and `docs/architecture/`
- supporting asset directories under `docs/assets/`, `docs/images/`, and `docs/screenshots/`

Execution note:
- Recommendations below were captured against the pre-cleanup layout.
- Archive moves and active-doc link fixes were applied on 2026-03-07; entries keep original paths so the triage decisions remain auditable.

Legend:
- `Current` = accurate enough to keep in the active docs set
- `Stale` = outdated but potentially still useful
- `Internal-only` = development/design/audit artifact, not part of the active user/contributor docs surface
- `Archive candidate` = historical value, should move under `docs/archive/`

## Prose and config docs

| File path | Classification | Recommended action |
|---|---|---|
| `docs/README.md` | Stale | Update index to remove archived/design/audit links and point only at current docs. |
| `docs/GUIDELINES.md` | Current | Keep, but clarify archive placement and current docs categories. |
| `docs/getting-started.md` | Current | Keep in active docs. |
| `docs/coordination-architecture.md` | Current | Keep in active docs; update links if archived protocol/design docs move. |
| `docs/team-templates.md` | Current | Keep in active docs. |
| `docs/file-rendering-pipeline.md` | Current | Keep in active docs. |
| `docs/platform-abstraction.md` | Current | Keep in active docs. |
| `docs/architecture-drift-report.md` | Internal-only | Archive under `docs/archive/architecture/`. |
| `docs/feature-matrix.md` | Stale | Archive; version/status table is already drifting and duplicates feature docs/changelog. |
| `docs/testing-guide.md` | Internal-only | Move into `docs/operations/` as a visual-testing guide, or archive if not maintained. |
| `docs/ai-agent-characteristics.md` | Internal-only | Archive. |
| `docs/bun-migration-guide-2026-03-05.md` | Archive candidate | Archive. |
| `docs/design-stall-detection.md` | Internal-only | Archive. |
| `docs/design-workflow.md` | Internal-only | Archive. |
| `docs/e2e-performance-bug.md` | Archive candidate | Archive. |
| `docs/mesh-design-vision.md` | Archive candidate | Archive. |
| `docs/mesh-setup-vision.md` | Archive candidate | Archive. |
| `docs/mesh-team-setup-refactor-plan.md` | Archive candidate | Archive. |
| `docs/mesh-view-design.md` | Archive candidate | Archive. |
| `docs/phase-4-architecture.md` | Archive candidate | Archive. |
| `docs/readme-gap-analysis.md` | Internal-only | Archive under `docs/archive/planning/`. |
| `docs/readme-content-plan.md` | Internal-only | Archive under `docs/archive/planning/`. |
| `docs/retro-quality-sprint-2026-03-05.md` | Archive candidate | Archive. |
| `docs/screenshot-shot-list.md` | Internal-only | Archive under `docs/archive/planning/`. |
| `docs/security-audit-task56-2026-03-04.md` | Archive candidate | Archive. |
| `docs/features/command-center.md` | Current | Keep in active docs. |
| `docs/features/file-browser.md` | Current | Keep in active docs. |
| `docs/features/first-run-and-settings.md` | Current | Keep in active docs. |
| `docs/features/git-integration.md` | Current | Keep in active docs. |
| `docs/features/mesh.md` | Current | Keep in active docs, but remove links to archived design docs. |
| `docs/features/project-management.md` | Current | Keep in active docs. |
| `docs/features/search.md` | Current | Keep in active docs. |
| `docs/features/session-management.md` | Current | Keep in active docs. |
| `docs/features/task-board.md` | Current | Keep in active docs. |
| `docs/operations/build-and-release.md` | Current | Keep in active docs. |
| `docs/operations/testing-guide.md` | Current | Keep in active docs. |
| `docs/ui/design-system.md` | Current | Keep in active docs. |
| `docs/ui/layout-and-navigation.md` | Current | Keep in active docs. |
| `docs/ui/project-hovercard-ui-concept.md` | Internal-only | Archive. |
| `docs/ui/project-hovercard-vision.md` | Internal-only | Archive. |
| `docs/security/risk-register.md` | Current | Keep in active docs. |
| `docs/security/audit-2026-02-27.md` | Archive candidate | Archive under `docs/archive/security/`. |
| `docs/security/sec-auditor-audit-2026-03-03.md` | Archive candidate | Archive under `docs/archive/security/`. |
| `docs/security/team-lead-audit-2026-03-03.md` | Archive candidate | Archive under `docs/archive/security/`. |
| `docs/architecture/daemon-protocol.md` | Current | Keep in active docs. |
| `docs/architecture/data-model.md` | Current | Keep in active docs. |
| `docs/architecture/ipc-reference.md` | Current | Keep in active docs, but replace link to archived mesh design doc. |
| `docs/architecture/log-level-guidelines.md` | Current | Keep in active docs. |
| `docs/architecture/logging-design.md` | Current | Keep in active docs. |
| `docs/architecture/mesh-versioning-strategy.md` | Current | Keep in active docs. |
| `docs/architecture/orchestration-practical-auto-idle-and-communication.md` | Current | Keep in active docs. |
| `docs/architecture/template-storage-git2.md` | Current | Keep in active docs. |
| `docs/architecture/daemon-upgrade-migration.md` | Archive candidate | Archive. |
| `docs/architecture/lightweight-visual-testing-approach.md` | Archive candidate | Archive; superseded by active testing docs. |
| `docs/architecture/logging-integration-point-inventory.md` | Internal-only | Archive. |
| `docs/architecture/mesh-agent-resume-architecture.md` | Archive candidate | Archive. |
| `docs/architecture/mesh-canvas-layout-engine-concept.md` | Archive candidate | Archive. |
| `docs/architecture/mesh-canvas-library-assessment.md` | Archive candidate | Archive. |
| `docs/architecture/mesh-tab-load-analysis.md` | Internal-only | Archive. |
| `docs/architecture/orchestration-protocol-design.md` | Archive candidate | Archive; keep referenced as historical protocol exploration from coordination architecture. |
| `docs/audits/agent-team-lifecycle-robustness-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/ai-friendliness-audit-claude.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/ai-friendliness-audit-codex.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/ci-enforcement-assessment-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/code-quality-audit-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/daemon-hot-swap-integration-gap-analysis-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/e2e-gap-assessment.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/mesh-daemon-availability-reliability-audit-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/package-manager-recommendation-2026-03-05.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/perf-audit-backend.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/perf-audit-frontend.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/audits/shell-decomposition-assessment-2026-03-07.md` | Archive candidate | Archive entire `docs/audits/` tree. |
| `docs/design/agent-role-visibility.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/cross-project-member-distinction.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/glass-depth-exploration.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/role-context-steering-review.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/sidebar-icon-refinement.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/sidebar-session-grouping.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/sidebar-team-session-visuals.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/team-lifecycle-ux.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/design/team-resume-lifecycle.md` | Internal-only | Archive entire `docs/design/` tree. |
| `docs/research/auto-pane-id-reconciliation.md` | Internal-only | Archive entire `docs/research/` tree. |
| `docs/research/codex-mcp-integration.md` | Internal-only | Archive entire `docs/research/` tree. |
| `docs/research/rust-build-cleanup.md` | Internal-only | Archive entire `docs/research/` tree. |
| `docs/retros/layout-engine-pipeline-retro.md` | Archive candidate | Archive entire `docs/retros/` tree. |
| `docs/retros/visual-testing-pipeline-lessons.md` | Archive candidate | Archive entire `docs/retros/` tree. |
| `docs/images/infographics.manifest.yaml` | Internal-only | Keep with current images; not user-facing, but still an active support manifest. |

## Supporting assets

| File path / group | Classification | Recommended action |
|---|---|---|
| `docs/screenshot-overview.png` | Current | Keep; referenced by root README. |
| `docs/screenshot-git.png` | Current | Keep; referenced by root README. |
| `docs/screenshot-files.png` | Current | Keep; referenced by root README. |
| `docs/images/*.jpg` excluding `images/mesh-redesign/` | Current | Keep; these back active docs and architecture pages. |
| `docs/assets/grouped-icons/**` | Archive candidate | Archive; design exploration assets, not active docs surface. |
| `docs/assets/grouped-icons-v2/**` | Archive candidate | Archive; design exploration assets, not active docs surface. |
| `docs/images/mesh-redesign/**` | Archive candidate | Archive with historical mesh design material. |
| `docs/screenshots/**` | Archive candidate | Archive; iteration screenshots and bug snapshots, not active docs assets. |

## Missing docs / follow-up candidates

| Missing or follow-up item | Why it should exist | Recommended action |
|---|---|---|
| `docs/operations/mesh-troubleshooting.md` | Mesh is prerequisite-heavy and failure-prone, but troubleshooting is scattered across `getting-started.md`, feature docs, and architecture docs. | Flag for follow-up documentation task. |
| Split of `docs/coordination-architecture.md` into current overview + archived decision log | The current doc is accurate but large and mixes current architecture with historical decision log detail. | Flag for follow-up rewrite/split task, not immediate cleanup. |
