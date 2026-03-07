# README gap analysis

Date: 2026-03-07
Target release: taurhaus v0.5.5
Task: #593

## Executive summary

The current [README.md](/home/mstie/projects/taurhaus/README.md) is not wrong, but it reads like a smaller and earlier product than taurhaus actually is in `v0.5.5`.

The main problem is not factual breakage. The main problem is that the README still presents taurhaus as a clever side-panel for juggling AI sessions, while the shipped product is now a broader operations surface for:

- supervising live multi-tool sessions across many projects
- inspecting project context quickly
- aggregating work and handoff history
- coordinating persistent multi-agent Mesh teams with recovery workflows

That mismatch creates four gaps:

1. the README undersells the product's maturity
2. major workflows are either missing or compressed into one-line bullets
3. current screenshots only cover passive browsing views
4. the tone still carries early-project self-deprecation instead of operator confidence

## 1. Features missing or materially undersold

### A. Session control is stronger than the README suggests

The README mentions multi-CLI session management, but it does not convey that taurhaus is an active command center, not just a passive status panel.

What is missing:

- per-tool launch modes: continue, fresh, resume
- stop and restart flows for running sessions
- click-to-focus tmux session navigation
- platform-aware terminal focus/open behavior
- configurable per-tool launch commands and tmux layout behavior

Source of truth:

- [docs/features/command-center.md](/home/mstie/projects/taurhaus/docs/features/command-center.md)
- [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md)

Why this matters:

The current README makes taurhaus sound like a dashboard. The actual product can also initiate and control work.

### B. Session continuity is deeper than “see what’s running”

The README mentions session handoffs and live activity detection, but it does not explain the continuity model:

- event-driven live session updates
- active/idle classification per tool
- session history grouped by archived sessions
- handoff summaries with next steps and open questions
- commit/file enrichment around session windows

Source of truth:

- [docs/features/session-management.md](/home/mstie/projects/taurhaus/docs/features/session-management.md)
- [docs/features/task-board.md](/home/mstie/projects/taurhaus/docs/features/task-board.md)

Why this matters:

“I can recover context fast” is one of the strongest user-value claims taurhaus can make, and the current README does not make it clearly enough.

### C. Mesh is far beyond “live roster + hot-add”

The current README describes Mesh as a view to initialize teams, track status, and hot-add members. That is now incomplete.

What is missing:

- availability gate for mesh/tmux/tool prerequisites
- role and preset templates with compose/apply flow
- runtime canvas and node detail actions
- remove-member teardown
- re-onboard flows
- member resume
- team resume after cold restart
- degraded/cold-resume detection and recovery affordances
- disband cleanup and stale-team discovery

Source of truth:

- [docs/features/mesh.md](/home/mstie/projects/taurhaus/docs/features/mesh.md)
- [docs/coordination-architecture.md](/home/mstie/projects/taurhaus/docs/coordination-architecture.md)
- [CHANGELOG.md](/home/mstie/projects/taurhaus/CHANGELOG.md)

Why this matters:

Mesh is now one of the clearest differentiators of taurhaus. The README currently treats it as an add-on instead of a flagship workflow.

### D. Team templates and role context steering are barely visible

The current README mentions built-in role/preset catalog and composition flow, but only as one bullet. It does not explain why templates matter:

- reusable team setups
- role-driven context steering
- role/preset composition before initialize
- template history, diff, revert, and storage status

Source of truth:

- [docs/team-templates.md](/home/mstie/projects/taurhaus/docs/team-templates.md)
- [docs/design/role-context-steering-review.md](/home/mstie/projects/taurhaus/docs/design/role-context-steering-review.md)
- [ARCHITECTURE.md](/home/mstie/projects/taurhaus/ARCHITECTURE.md)

Why this matters:

Without this, the README reduces Mesh setup to manual roster assembly and misses a substantial product surface.

### E. The task board is undersold

The README says “aggregated tasks from Claude Code, Codex, and Gemini CLI in one view,” which is true but thin.

What is missing:

- normalized cross-tool task model
- active vs history views
- dependencies, owners, and session metadata
- archived-session grouping
- commit/file context enrichment in task detail

Source of truth:

- [docs/features/task-board.md](/home/mstie/projects/taurhaus/docs/features/task-board.md)

Why this matters:

This is not just a task list. It is a per-project work ledger across multiple agent systems.

### F. Search is presented too generically

The current README says “full-text search across all project content,” but does not explain what that includes or how it navigates.

What is missing:

- documents + sessions + commits in one overlay
- grouped result types
- cross-project navigation behavior
- tantivy-backed index lifecycle and rebuild support

Source of truth:

- [docs/features/search.md](/home/mstie/projects/taurhaus/docs/features/search.md)

### G. Project management and onboarding are more complete than the README shows

The current README mentions scan/register existing repos and create a new git project, but it still compresses core project-management behavior too much.

What is missing:

- quick scan, manual add, and first-run batch import as distinct flows
- activity-group behavior with configurable thresholds
- relationships surfaced in Overview
- Manage Projects workflows and validation behavior

Source of truth:

- [docs/features/project-management.md](/home/mstie/projects/taurhaus/docs/features/project-management.md)
- [docs/features/first-run-and-settings.md](/home/mstie/projects/taurhaus/docs/features/first-run-and-settings.md)

## 2. Outdated or misleading content

### A. The voice still reads like an early prototype

Examples:

- “deeply unwise workflow”
- “we’re not here to judge”
- “If anything, it enables the problem”
- “Built by someone with the same problem. You’re among friends here.”

None of that is technically false, but it positions taurhaus as a clever side project instead of a serious tool. That is now outdated relative to the shipped product quality and scope.

### B. The feature list is flat and pre-maturity

The current “Features” section is a single mixed bullet list that combines:

- workflows
- implementation details
- major differentiators
- small supporting capabilities

That structure was acceptable earlier. At `v0.5.5`, it now hides product shape instead of clarifying it.

### C. Development guidance is stale/incomplete for current contributor workflow

The current Development section is usable, but it misses important current reality:

- Bun-only workflow
- `just test-visual`
- Linux/macOS E2E split
- remote macOS build path
- team/agent expectation that `just check-quick` is the normal implementation gate and `just check` is serialized

Source of truth:

- [CLAUDE.md](/home/mstie/projects/taurhaus/CLAUDE.md)
- [CONTRIBUTING.md](/home/mstie/projects/taurhaus/CONTRIBUTING.md)

### D. Mesh recovery and resilience are absent from product framing

Recent `0.5.4` and `0.5.5` changes turned reliability into a product-level benefit:

- Resume Team after cold restart
- degraded/offline recovery
- daemon hot-swap
- non-blocking Mesh runtime refresh

These do not need deep technical explanation in the README, but their user-facing outcome should be mentioned. Right now they are invisible.

## 3. Messaging gaps

### A. The README leads with personality before value

The opening copy is memorable, but it does not answer the most important first question cleanly enough:

“Why would I install taurhaus instead of just living in tmux and my editor?”

The README should lead with:

- operational visibility across projects and tools
- faster context recovery
- coordinated multi-agent workflows

### B. The README does not clearly state the main value pillars

The real product messaging should cluster around a few strong ideas:

1. **Know what every agent is doing right now**
2. **Recover project context without terminal archaeology**
3. **Coordinate multi-agent teams without losing operational control**

The current README mentions all three indirectly, but never states them as the product’s core proposition.

### C. It reads more like “tool list” than “workflow system”

The current copy names tabs and features, but not enough end-to-end workflows:

- start or resume a session
- inspect README/commits/tasks/handoffs for context
- search across projects when context is fragmented
- launch and recover a Mesh team

### D. There is not enough proof of maturity

The code/docs/changelog show:

- 80+ IPC commands
- dedicated daemon
- cross-platform tmux/session management
- full Mesh lifecycle
- visual testing lane
- structured logging and recovery work

The README does not need to list all of that, but it should sound like a mature operator tool, not a promising prototype.

## 4. Screenshot assessment

## Current screenshots

The current README includes:

- overview screenshot
- git screenshot
- files screenshot
- system architecture diagram

## What these screenshots do well

- They prove taurhaus is a real desktop app.
- They show that the app is denser and more technical than a typical marketing landing page.
- They cover project inspection reasonably well.

## What is missing

The screenshot set does not cover the product’s strongest differentiators:

- sidebar with live session indicators and project activity groups
- task board
- search overlay
- Mesh setup flow
- Mesh runtime canvas / agent detail actions
- team templates / customization flow
- settings / onboarding only if needed for setup confidence

## Main issue

The current screenshot set makes taurhaus look like a project browser with git and file views. It does not visually prove:

- session supervision
- work aggregation
- multi-agent coordination
- recovery/continuity workflows

## Architecture diagram placement issue

The architecture diagram is useful, but it should not carry as much visual weight as product screenshots in the main README flow. It belongs lower in the page, near architecture/development, not as one of the primary product proof assets.

## 5. Structure issues

### A. “What this is / what this isn’t” is too long for the current stage

This section spends too much of the page budget on framing and jokes before the README shows the product’s strongest workflows.

### B. The README should be organized by user workflow, not by miscellaneous capability bullets

Better grouping would be:

- supervise sessions and projects
- inspect context and work history
- coordinate teams in Mesh
- install and get started
- contribute / develop

### C. Installation arrives before enough proof

The README gets to Setup after one flat feature list. For a product with strong visuals and multiple workflows, that is too early. The reader should first see:

- what taurhaus is
- why it matters
- how it looks
- what the core workflows are

### D. Architecture and development sections are too compressed

The current README technically links out, but it does not distinguish clearly between:

- end-user install/use guidance
- contributor workflow/build guidance
- technical architecture overview

These should stay in the README, but with clearer separation and stronger scoping.

## Bottom line

The current README undersells taurhaus in three specific ways:

1. it sounds more casual and provisional than the product actually is
2. it visually proves only browsing workflows, not the orchestration workflows
3. it does not present taurhaus as a workflow system with continuity and coordination, only as a panel of features

That means the next README should not just be a copy edit. It should be a structural rewrite around workflow-driven messaging and stronger visual proof.
