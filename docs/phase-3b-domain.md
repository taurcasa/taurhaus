# Phase 3B: Domain Understanding

> Extracted from the [Design Brief](design-brief.md). Structured for direct consumption by Phase 3C (User Journey Mapping) and Phase 3D (Information Architecture).

---

## Entity Inventory

### E-01: Project

**Display name**: Project name (directory name by default, user-overridable). Disambiguated by path when names collide.

**Key properties** (ranked by user importance):
1. **Name** — primary identifier, what distinguishes one project from another
2. **Working tree status** — clean/dirty signal, immediate visual indicator of uncommitted work
3. **Current branch** — where development is happening
4. **Last activity date** — recency signal, drives "where should I focus?" decisions
5. **Description** — what the project is/does
6. **Project type / tags** — categorization (Tauri app, knowledge base, MCP server, CLI tool)
7. **Path** — filesystem location, secondary identifier
8. **Relationships** — links to other projects (summary count or list)

**States**: Activity-derived, not manually managed.
- **Active**: Commits within the last 7 days
- **Recent**: Commits within the last 30 days
- **Stale**: No commits in 30-90 days
- **Dormant**: No commits in 90+ days
- Optional user-set status tag (overrides or supplements derived state)

> State thresholds are suggestions for the UI — the exact boundaries are a design decision for 3E. The key insight is that state is derived from git activity, not user input.

**Relationships**:
- Has many Sessions (one-to-many)
- Has many Documents (one-to-many)
- Has many Relationships as source or target (many-to-many via E-04)

**Volume**: Min 15, typical 30-50, max 100+. Growth: steady, all AI projects added over time.

**Active working set**: All projects are visible simultaneously on the dashboard. No "active vs. archived" filter needed for v1 — the activity-derived state provides natural visual prioritization.

---

### E-02: Session / Handoff

**Display name**: Date + project name (e.g., "2026-02-16 — taurhaus"). Most recent session for a project is visually prominent.

**Key properties** (ranked by user importance):
1. **Summary** — what happened this session (the "what did we do?" answer)
2. **Next steps** — what to do next (the "where do we pick up?" answer)
3. **Open questions** — unresolved items that need attention
4. **Date/time** — when this session occurred
5. **Decisions made** — choices recorded during the session
6. **User notes** — personal annotations, diagrams, images added in taurhaus
7. **Claude Code session ID** — reference back to the original session (if available)

**States**:
- **Created**: Handoff file written by Claude Code skill (exists on disk, not yet seen by taurhaus)
- **Imported**: Auto-detected by file watcher, parsed and indexed
- **Current**: Most recent session for its project (automatic — newest wins)
- **Historical**: Superseded by a newer session

**State transitions**: All system-initiated. Created → Imported (file watcher). Imported → Current (automatic). Current → Historical (superseded).

**Relationships**:
- Belongs to one Project (many-to-one)
- Supersedes previous session for same project (implicit ordering by date)
- May reference Documents (handoff files link to project docs)

**Volume**: Min 0 per project, typical 5-20 per active project, max hundreds. Growth: continuous, one per session.

**Active working set**: Usually only the most recent 1-3 sessions per project matter. Historical sessions are searchable but not prominently displayed.

---

### E-03: Document

**Display name**: File name, shown with relative path when context is needed (e.g., "design-brief.md" or "docs/design-brief.md").

**Key properties** (ranked by user importance):
1. **File name** — primary identifier
2. **File type** — determines rendering (markdown → rendered, source → syntax-highlighted, image → displayed)
3. **Relative path** — location within project, provides tree structure
4. **Last modified date** — recency signal
5. **Size** — rough indicator of content scope
6. **Content** — the actual file content (for rendering and search indexing)

**States**: Filesystem-tracked, no user-managed states.
- **Present**: File exists on disk
- **Modified**: File changed since last index (transient — triggers re-index, then returns to Present)
- **Deleted**: File removed from disk (removed from index)

**State transitions**: All system-initiated via file watcher.

**Relationships**:
- Belongs to one Project (many-to-one)
- Referenced by Sessions (handoff docs may link to specific files)
- Organized in a file tree (parent directory → child files/directories)

**Volume**: Min ~20 per project (small KB), typical 50-200, max thousands (large codebase). Filtered by ignore patterns (`.gitignore` + `.taurhausignore`).

**Active working set**: Varies by usage. Markdown docs and config files are frequently accessed. Source files less so (viewed, not edited). The file tree provides navigation; search provides direct access.

---

### E-04: Relationship

**Display name**: "Source → Target (type)" (e.g., "taurui → MIR (provides design to)").

**Key properties** (ranked by user importance):
1. **Relationship type** — what kind of link (provides design to, audited by, depends on, forked from)
2. **Source project** — where the relationship originates
3. **Target project** — where the relationship points
4. **Description** — optional freetext elaboration

**States**:
- **Active**: Link exists and is displayed
- **Removed**: Link deleted by user

**State transitions**: All user-initiated. Create and Remove only.

**Relationships**:
- Connects exactly two Projects (directional: A→B is different from B→A)

**Volume**: Min 0 per project, typical 2-5, max ~20. Sparse graph. Total across all projects: probably 20-80 relationships.

**Active working set**: All active relationships are always relevant (small volume, no filtering needed).

---

## Action Vocabulary

### Project Actions

| Action | Type | Trigger | Notes |
|--------|------|---------|-------|
| Browse all | Read | Dashboard load | Default view. Shows all projects. |
| View detail | Read | Click project in list | Opens project deep-dive. |
| Register | Create | Manual (add by path) or batch (scan ~/projects/) | Adds project to taurhaus registry. |
| Remove | Delete (soft) | User action + confirmation | Unregisters project. Does NOT delete files. User-added metadata may be lost. |
| Edit metadata | Update | Inline or form | Change name, description, tags. |
| Link to project | Create (Relationship) | User action | Creates a Relationship entity (E-04). |
| Filter / sort | Read | UI controls | By activity state, tags, name, last activity date. |
| Search | Read | Search input | Name, description, tags. Part of global search. |

### Session Actions

| Action | Type | Trigger | Notes |
|--------|------|---------|-------|
| View list | Read | Navigate to project sessions | Chronological list per project. |
| View detail | Read | Click session | Full handoff content. |
| Import | Create (system) | File watcher detects handoff file | Automatic. No user action. |
| Add notes/images | Update | User action in session detail | Enrich the handoff with personal annotations. |
| Search | Read | Search input | Across session summaries, decisions, next steps. Part of global search. |

### Document Actions

| Action | Type | Trigger | Notes |
|--------|------|---------|-------|
| Browse tree | Read | Navigate to project docs | File tree navigation. |
| View / render | Read | Click file | Markdown rendered, source syntax-highlighted, images displayed. |
| Search | Read | Search input | Full-text content search. Part of global search. |

### Relationship Actions

| Action | Type | Trigger | Notes |
|--------|------|---------|-------|
| Create | Create | User action from project detail | Define type, select target project. |
| Edit | Update | User action | Change type or description. |
| Remove | Delete | User action + confirmation | Delete the link. Low stakes. |
| View | Read | Project detail or graph visualization | See all relationships for a project. |

---

## Design Constraints

### Data Access Model

- **All data is local.** Tauri IPC commands (invoke) to Rust backend. No network latency.
- **Git data via libgit2.** Commit history, branches, status, diffs — all read via Rust.
- **File data via filesystem.** File watcher (notify crate or similar) for change detection.
- **Index via Rust backend.** Full-text search index built and maintained in Rust. Queried via IPC.

### Performance Characteristics

| Operation | Expected latency | UI implication |
|-----------|-----------------|----------------|
| List all projects | <50ms | Instant. No loading state needed. |
| Project detail (metadata + recent commits) | <100ms | Near-instant. Subtle loading state if needed. |
| Full commit history (large repo) | 100ms-2s | Loading state required. Show recent first, load rest progressively. |
| File tree for project | <100ms | Near-instant. |
| Render markdown file | <50ms | Instant. |
| Render large source file | 50-200ms | May need progressive rendering for very large files. |
| Full-text search | <200ms | Fast enough for search-as-you-type with debounce. |
| Initial project scan (all projects) | 2-10s | Progress indicator required. First-run experience. |
| Index rebuild | 5-30s | Background operation. UI remains responsive. Progress indicator. |

### Filtering and Sorting

- All client-side (Rust backend). No server-side pagination needed.
- Projects: sort by name, last activity, status. Filter by activity state, tags.
- Sessions: sort by date (default: newest first). Filter by project.
- Documents: navigate via tree. Search via full-text index.
- Commits: sort by date (default: newest first). Filter by author, message text.

### Real-Time Updates

- **File watcher** pushes events to the frontend via Tauri event system.
- Dashboard updates automatically when files change, commits are made, or handoff files appear.
- No polling. Event-driven reactivity.

### Async Operations Requiring UI Feedback

1. **Initial project scan** — first-run. Progress bar or step indicator showing projects discovered/indexed.
2. **Index rebuild** — triggered manually or on corruption. Background progress indicator.
3. **Git history fetch** — per-project, on first view of large repos. Inline loading state.
4. **File watcher initialization** — setting up watchers on all projects. Brief startup delay.

### Error Categories and UI Treatment

| Category | Examples | UI treatment |
|----------|----------|-------------|
| Filesystem | Path not found, permission denied, disk full | Inline error in affected view. "Project path not found: /home/..." with suggested action. |
| Git | Not a git repo, corrupted repo, detached HEAD | Degrade gracefully. Show available data. Badge/indicator for issues. |
| Index | Corrupt index, stale index | Self-healing. Auto-rebuild. Brief "Rebuilding index..." indicator. |
| Parse | Malformed handoff file, unreadable file | Skip with warning. Don't block other content. |

**No network errors.** No auth errors. No rate limiting. Fully local application.

### Scope Constraints (What the UI Cannot Do)

- Cannot edit source code
- Cannot run builds, tests, or deployments
- Cannot create or modify files on disk (except taurhaus's own data — session notes, metadata)
- Cannot manage users (v1 single-user)
- Cannot manage tasks (deferred to v1.1+)

---

## Role-Permission Matrix

### v1: Developer (sole role)

| Entity | Browse | View | Create | Edit | Delete |
|--------|--------|------|--------|------|--------|
| Project | Yes | Yes | Yes (register) | Yes (metadata) | Yes (unregister) |
| Session | Yes | Yes | No (system) | Yes (add notes) | No |
| Document | Yes | Yes | No (read-only) | No | No |
| Relationship | Yes | Yes | Yes | Yes | Yes |
| Search | Yes | — | — | — | — |
| Settings | — | Yes | — | Yes | — |

### Future: Viewer (read-only)

| Entity | Browse | View | Create | Edit | Delete |
|--------|--------|------|--------|------|--------|
| Project | Yes | Yes | No | No | No |
| Session | Yes | Yes | No | No | No |
| Document | Yes | Yes | No | No | No |
| Relationship | Yes | Yes | No | No | No |
| Search | Yes | — | — | — | — |
| Settings | — | No | — | No | — |

### Future: Admin

| Entity | Browse | View | Create | Edit | Delete |
|--------|--------|------|--------|------|--------|
| Project | Yes | Yes | Yes | Yes | Yes |
| Session | Yes | Yes | No (system) | Yes | Yes |
| Document | Yes | Yes | No | No | No |
| Relationship | Yes | Yes | Yes | Yes | Yes |
| Search | Yes | — | — | — | — |
| Settings | — | Yes | — | Yes | — |
| Users | Yes | Yes | Yes | Yes | Yes |

> v1 implements Developer only. The architecture should use a permission check pattern that can be extended to Viewer/Admin without retrofitting.

---

## Implicit Entities (Not User-Facing)

These exist in the system but are not directly managed by the user:

- **Git Commit**: Displayed within project detail (commit log). Not a standalone entity the user navigates to independently. Properties: hash, message, author, date, diff.
- **Git Branch**: Displayed as a property of Project. Not independently managed.
- **File Watcher State**: System-internal. User sees its effects (real-time updates) but never interacts with it directly.
- **Search Index**: System-internal. User searches against it but never manages it (auto-maintained).
- **Settings / Configuration**: User-facing but not an entity in the traditional sense. Global preferences (ignore patterns, display options). Accessed via a settings view.

---

## Handoff to Phase 3C

This document provides the raw material for journey mapping:

- **Entity inventory** (E-01 through E-04) → entity walk-through in 3C Step 2
- **Action vocabulary** → journey candidate identification
- **Volume estimates** → frequency and volume scoring in 3C Step 3
- **Design constraints** → what's technically feasible for journey steps
- **Role-permission matrix** → user role definition in 3C Step 1
- **Implicit entities** (commits, branches) → appear as data within journeys, not as journey subjects
