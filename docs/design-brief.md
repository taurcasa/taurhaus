# taurhaus — Design Brief

> The house where all your projects live.

This document is the formal output of **Phase 2 (Requirements)** of the TaurUI project lifecycle. It serves as the primary input for Phase 3 (UI Design) and all downstream phases. Compiled from interactive questionnaire sessions — not assumptions.

---

## 1. Product Overview

**Product name**: taurhaus

**One-sentence description**: A desktop tool that gives a single, clear view into all AI-driven projects — their code, docs, progress, and history — so you never lose context between sessions.

**Problem statement**: With 15+ projects across `~/projects/`, the only way to see project state is `git log`, `ls`, and memory. Cross-project relationships (taurui feeds MIR's design, taursec audits MIR's code) are invisible. Session handoffs rely on markdown files that no tool renders. Starting a new coding session means minutes of manual archaeology instead of seconds of orientation.

**Scope boundaries**:

- **No code editing** — taurhaus is read-only for source code. Viewing: yes. Editing: no.
- **No CI/CD** — does not build, deploy, or run tests. That stays in the terminal.
- **No real-time collaboration** — single-user tool for v1.
- **Task management**: deferred (maybe v1.1, definitely future). Architecture must not preclude it.
- **User management**: single user for v1. Data model and permissions architecture must support multi-user in the future without a retrofit.

---

## 2. Users

### Role: Developer (Primary — solo, v1 only)

- **Who they are**: A developer managing 15-50 AI-assisted projects in parallel. High technical sophistication. Works primarily in Claude Code terminal sessions.
- **What they care about**: Orienting quickly — "what's the state of everything?" (highest frequency). Resuming context — "what did we do last time?" (session start). Tracking progress — "how far along is each project?" (less frequent).
- **How often they use it**: Kept open alongside Claude Code as a persistent reference. Hours at a time, glanced at throughout the day. Not a launcher — a companion.
- **Context of use**: Desktop, ultrawide monitor (5120x1440). Three-zone layout: center focus (2560x1440), left/right side panels (1280x1440 each). taurhaus typically sits in a side panel, occasionally in center.

### Future Roles (not v1, but architecture must accommodate)

- **Viewer**: Read-only access to project status and docs. No edit permissions.
- **Admin**: Manages project registry, user permissions, global settings.

---

## 3. Entities

### Project

- **What it is**: A flexible container for any kind of project (Tauri app, knowledge base, MCP server, game engine, CLI tool). Always git-backed — git metadata is available for every project.
- **Key properties**: Name, path, description, project type/tags, last activity date, current branch, working tree status (clean/dirty), relationships to other projects.
- **Display name format**: Project name (directory name by default, user-overridable). Disambiguated by path when names collide.
- **Lifecycle / states**: Primarily derived from activity (last commit date, commit frequency) rather than manually managed. Optional user-set status tag, but no forced lifecycle. Projects organically gain and lose focus.
- **Relationships**: Typed + directional links to other projects (see Relationship entity).
- **Volume**: Min 15, typical 30-50, max 100+. Growing — all AI-driven projects will be tracked.

### Session / Handoff

- **What it is**: A record of a Claude Code working session — what happened, decisions made, context for next time. First-class entity, not just a markdown file.
- **Key properties**: Project (belongs to), date/time, summary, decisions made, open questions, next steps, Claude Code session ID (if available).
- **Display name format**: Date + project name (e.g., "2026-02-16 — taurhaus"). Most recent session is visually prominent.
- **Lifecycle / states**: Created (via Claude Code skill) → Imported (auto-detected by taurhaus) → Current (most recent for a project) → Historical.
- **State transitions**: Created → Imported is system-initiated (file watcher detects new handoff file). Imported → Current is automatic (most recent wins). Current → Historical is automatic (superseded by newer session).
- **Relationships**: Belongs to one Project. Supersedes previous session for that project.
- **Volume**: Min 0 per project, typical 5-20 per active project, max hundreds for long-running projects. Created every session, so grows continuously.
- **Creation mechanism**: NOT manual entry. Created via a custom Claude Code skill that writes structured handoff files. taurhaus auto-detects and imports these as first-class session records. User may also add personal notes, diagrams, or images directly in taurhaus.

### Document

- **What it is**: Any viewable file within a project — markdown docs, source code (read-only), images, config files. Browsable and searchable.
- **Key properties**: File path (relative to project), file type, last modified date, size, content (for indexing/search).
- **Display name format**: File name, shown with relative path when context is needed.
- **Lifecycle / states**: Tracks the file on disk. Changes detected via file watcher. No user-managed states.
- **State transitions**: All system-initiated — file created, modified, or deleted on disk triggers corresponding update.
- **Relationships**: Belongs to one Project. Referenced by Sessions (handoff docs).
- **Volume**: Varies wildly per project. Small knowledge base: ~20 files. Large codebase: thousands. File watcher must have configurable ignore patterns (`.gitignore` as baseline, plus `.taurhausignore` or config for additional filtering).

### Relationship

- **What it is**: A typed, directional link between two projects.
- **Key properties**: Source project, target project, relationship type (e.g., "provides design to", "audited by", "depends on", "forked from"), description (optional).
- **Display name format**: "Source → Target (type)" (e.g., "taurui → MIR (provides design to)").
- **Lifecycle / states**: Created → Active → Removed. No complex lifecycle.
- **State transitions**: Created and Removed are user-initiated. No automatic transitions.
- **Relationships**: Connects exactly two Projects. Directional (A→B is different from B→A).
- **Volume**: Min 0, typical 2-5 per project, max ~20. Sparse graph — not every project connects to every other.

---

## 4. Core Workflows

### Workflow: Orient Across Projects

- **Who**: Developer
- **Trigger**: Start of a work session, or switching mental context between projects.
- **Goal**: Decide which project to focus on next.
- **Rough steps**: 1. Glance at project dashboard. 2. See which projects have recent activity, which are stale. 3. Check if any project needs urgent attention. 4. Pick a project and drill in.
- **Key decisions**: "Where should I focus?" — needs to see recency of activity, project relationships, and any flags/notes.
- **How they know they're done**: A project is selected and they're looking at its detail view.
- **Frequency**: Multiple times daily. Variable entry — sometimes a full scan, sometimes direct to a known project, sometimes cross-project chronological review.

### Workflow: Resume Project Context

- **Who**: Developer
- **Trigger**: About to start a Claude Code session on a specific project.
- **Goal**: Understand where this project left off — what was done, what's pending, what decisions were made.
- **Rough steps**: 1. Open project detail. 2. Read most recent session handoff. 3. Scan recent git history for what changed. 4. Review any open questions or next steps. 5. Open Claude Code with context.
- **Key decisions**: "Is the last session's plan still valid, or has something changed?" — needs handoff summary + recent commits.
- **How they know they're done**: They have enough context to start a productive Claude Code session.
- **Frequency**: Every session start. 1-5 times daily.

### Workflow: Reference Docs Mid-Session

- **Who**: Developer
- **Trigger**: Working in Claude Code and needs to check a project document, source file, or image.
- **Goal**: Find and read the relevant document without leaving the current flow.
- **Rough steps**: 1. Switch to taurhaus (alt-tab or glance at side panel). 2. Navigate to the doc or search for it. 3. Read/view the content. 4. Switch back to Claude Code.
- **Key decisions**: None — this is a lookup, not a decision workflow.
- **How they know they're done**: They found the information they needed.
- **Frequency**: Highest frequency workflow. Many times per session. Must be fast — search or navigation should surface the right doc in seconds.

### Workflow: Search Across Projects

- **Who**: Developer
- **Trigger**: Needs to find something but doesn't remember which project it's in.
- **Goal**: Locate a specific piece of information — a pattern, a config, a decision, a commit.
- **Rough steps**: 1. Open search. 2. Type query. 3. See results across all projects — docs, commits, code. 4. Click into the relevant result.
- **Key decisions**: "Which result is the one I need?" — needs clear result context (which project, which file, matching snippet).
- **How they know they're done**: They found the thing.
- **Frequency**: Multiple times daily. v1 critical feature.
- **Performance**: Must be fast. Rust-powered full-text search across all indexed content. Intentionally designed, not a bolt-on.

### Workflow: End Session (Create Handoff)

- **Who**: Developer
- **Trigger**: Finishing a Claude Code session.
- **Goal**: Capture session context so the next session can pick up seamlessly.
- **Rough steps**: 1. In Claude Code, run the handoff skill. 2. Skill writes a structured handoff file. 3. taurhaus auto-detects and imports it. 4. Optionally, add personal notes/diagrams/images in taurhaus.
- **Key decisions**: What's worth noting beyond what the skill captures automatically.
- **How they know they're done**: Handoff is visible in taurhaus for the project.
- **Frequency**: End of every session. 1-5 times daily.

---

## 5. Volume and Scale

- **Total users**: 1 (v1). Architecture supports future multi-user.
- **Concurrent users**: 1.
- **Projects in view**: Typical 30-50, max 100+. Dashboard must handle this without pagination feeling necessary.
- **Docs per project**: 20 (small KB) to thousands (large codebase). File watcher with ignore patterns.
- **Git commits**: Recent ~50 shown by default. Full history searchable. Some projects may have thousands of commits.
- **Sessions per project**: 0-20 active, hundreds historical.
- **Growth trajectory**: Projects grow steadily. All AI projects will be added. Index must scale.
- **Real-time requirements**: File watcher detects changes to project files and git state. Dashboard updates without manual refresh. Not multi-user real-time — just local filesystem reactivity.
- **Indexing strategy**: Full upfront index on first scan. Background incremental updates as files change.

---

## 6. Constraints

### Platform

- **Target platform**: Desktop (Tauri 2).
- **Minimum viewport**: 1280x1440 (ultrawide side panel). Also used at 2560x1440 (center zone).
- **Offline requirements**: Fully local. No internet required. Reads filesystem and git repos directly.

### Technical

- **Tech stack**: Tauri 2 + Svelte 5 + Rust backend. Same stack as MIR (reference implementation).
- **Styling**: Tailwind v4 with custom design tokens.
- **API style**: Tauri IPC commands (invoke). No REST/GraphQL — all communication is local IPC.
- **Authentication**: None for v1 (single user). Architecture accommodates future auth.
- **Performance**: Search must be fast — Rust-powered full-text indexing. Upfront index, incremental updates. File watcher must handle large repos without excessive CPU/memory.

### Visual

- **Density**: Compact/dense. Screen real estate is shared with Claude Code.
- **Accessibility**: Best effort — focus on excellent UI/UX rather than formal WCAG compliance.
- **Font**: Geist Sans (consistent with MIR).

### Business

- **Timeline**: No hard deadline. Quality over speed.
- **Branding**: Greenfield. Part of the "taur-" ecosystem but no existing brand guidelines.
- **Existing applications**: No direct predecessor. MIR serves as a reference implementation for the tech stack and some UI patterns, but taurhaus is a different product.

---

## 7. Structural Artifacts

### Data Storage

Deferred to Architecture phase. Options to evaluate:

- **SQLite** (proven in MIR) — structured queries, good for indexing and search
- **Filesystem-first** (portable, git-trackable) — simpler, aligns with read-only philosophy
- **Hybrid** (SQLite for index, files for user content) — best of both worlds

### Claude Code Integration

- **Handoff skill**: Custom Claude Code skill that produces structured handoff files taurhaus can import. Skill design is part of taurhaus scope.
- **Session info**: Explore whether Claude Code session IDs can be captured and displayed in taurhaus (read-only session awareness).
- **IDE-style integration**: Explore whether Tauri can expose plugin/extension points similar to IDE integrations. Future investigation.
- **No MCP needed**: Claude Code sessions can write files directly to the filesystem. No intermediary protocol needed.

### File Watching

- Auto-detect changes to project files and git state.
- Configurable ignore patterns: `.gitignore` as baseline, plus project-level or global taurhaus-specific overrides (`.taurhausignore`).
- Must handle large codebases without excessive resource usage.

### User Actions by Entity

| Entity | Actions | Notes |
|--------|---------|-------|
| Project | View, Register, Remove, Edit metadata, Link | No destructive delete — "remove" just unregisters, doesn't touch filesystem. |
| Session | View, Import (auto), Add notes/images | Created externally. Import is system-initiated. |
| Document | View, Search | Read-only. No create/edit/delete within taurhaus. |
| Relationship | Create, Edit, Remove | User-managed. Lightweight — just metadata. |

### Destructive / Hard-to-Reverse Actions

- **Remove project from registry**: Reversible (re-register by path), but could lose user-added metadata. Confirm before executing.
- **Delete relationship**: Low-stakes but confirm to prevent accidental removal.
- **No filesystem-destructive actions**: taurhaus never deletes, moves, or modifies files on disk.

### Async Operations

- **Initial project scan**: Scanning all projects and building the index may take several seconds for large collections. Needs progress indication.
- **Full-text index rebuild**: Background operation. UI remains responsive.
- **Git operations**: Fetching commit history for large repos may take >100ms. Show loading state.

### Error Categories

- **Filesystem errors**: Path not found, permission denied, disk full. Surface clearly — user needs to fix the filesystem.
- **Git errors**: Not a git repo, corrupted repo, detached HEAD. Degrade gracefully — show what's available.
- **Index errors**: Corrupt index, out-of-date index. Self-healing — rebuild automatically.
- **No network errors**: Fully local application. No auth errors, no API errors.

---

## Completeness Check

- [x] Every user role is described with their primary goal and usage frequency
- [x] Every entity has properties, states, relationships, and volume estimates
- [x] Core workflows describe tasks (what users do), not features (what the UI has)
- [x] Volume numbers have ranges (min / typical / max), not single values
- [x] Constraints include platform, viewport, and accessibility requirements
- [x] Scope boundaries state what the application does NOT do

---

## Phase Transition

This Design Brief completes **Phase 2 (Requirements)**. Next step: **Phase 3 (UI Design)** — the TaurUI Full Design Procedure, sub-phases 3A through 3G.

### Phase naming convention

| taurhaus | TaurUI internal | Name |
|----------|----------------|------|
| 3A | Phase -1 | Brief Validation |
| 3B | Phase 0 | Domain Understanding |
| 3C | Phase 1 | User Journey Mapping |
| 3D | Phase 2 | Information Architecture |
| 3E | Phase 3 | View Design |
| 3F | Phase 4 | Visual System |
| 3G | Phase 5 | Specification |

Phase 3A (Brief Validation) is complete — 30/33 pass, 0 blocking. Next: Phase 3B.
