# taurhaus — Project Bootstrap

> The house where all your projects live.

## What This File Is

A bootstrap brief for starting taurhaus development. Follows the project lifecycle defined in `~/projects/taurui/process/project-lifecycle.md`.

### Phase Status

| Phase | Status | Output |
|-------|--------|--------|
| **Phase 1** — Planning | Complete | This file |
| **Phase 2** — Requirements | Complete | [`docs/design-brief.md`](docs/design-brief.md) |
| **Phase 3** — UI Design | Complete | See sub-phases below |
| ↳ **3A** Brief Validation | Complete | Validation results in design brief |
| ↳ **3B** Domain Understanding | Complete | [`docs/phase-3b-domain.md`](docs/phase-3b-domain.md) |
| ↳ **3C** User Journey Mapping | Complete | [`docs/phase-3c-journeys.md`](docs/phase-3c-journeys.md) |
| ↳ **3D** Information Architecture | Complete | [`docs/phase-3d-architecture.md`](docs/phase-3d-architecture.md) |
| ↳ **3E** View Design | Complete | [`docs/phase-3e-views.md`](docs/phase-3e-views.md) |
| ↳ **3F** Visual System | Complete | [`docs/phase-3f-visual.md`](docs/phase-3f-visual.md) |
| ↳ **3G** Specification | Complete | [`docs/phase-3g-specification.md`](docs/phase-3g-specification.md) |
| **Phase 4** — Architecture | Complete | [`docs/phase-4-architecture.md`](docs/phase-4-architecture.md) |
| **Phase 5** — Implementation | Complete | See sub-phases below |
| ↳ **5A** Scaffold + SQLite + Project CRUD | Complete | 7 tasks, 42 Rust + 16 JS tests |
| ↳ **5B** Git module + file reader | Complete | 6 tasks, 83 Rust + 30 JS tests |
| ↳ **5C** File watcher + session import | Complete | 6 tasks, 130 Rust + 55 JS tests |
| ↳ **5D** Search (tantivy) | Complete | 7 tasks, 165 Rust + 74 JS tests |
| ↳ **5E** Scanner + relationships | Complete | 6 tasks, 202 Rust + 80 JS tests |
| ↳ **5F** Claude Code integration | Complete | 6 tasks, 230 Rust + 80 JS tests |
| ↳ **5G** Polish + Settings + First-Run | Complete | 6 tasks, 246 Rust + 108 JS tests |
| ↳ **ADR-023** Dual-path architecture | Complete | 15 tasks, 356 Rust tests |

> **Naming convention**: Top-level phases use numbers (1-5). Sub-phases use letters (3A-3G). If a sub-phase needs further breakdown, use numbers again (3E.1, 3E.2). This avoids confusion with TaurUI's internal phase numbering (Phase 0-5), which maps to our 3B-3G.

---

## Phase 1: Planning

### Mission
A desktop tool that gives a single, clear view into all AI-driven projects — their code, docs, progress, and history — so you never lose context between sessions.

### Target User
A developer managing multiple AI-assisted projects in parallel. Works in Claude Code sessions that lose context. Needs to orient quickly: "what did we do last time?" and "what's the state of each project?"

### Core Problem
With 15+ projects across `~/projects/`, the only way to see project state is `git log`, `ls`, and memory. Cross-project relationships (TaurUI feeds MIR's design, TaurSec audits MIR's code) are invisible. Session handoffs rely on markdown files that no tool renders.

### Success Criteria
- Open taurhaus, see all active projects at a glance
- Click into a project: see recent git history, rendered docs, current status
- Understand cross-project relationships
- Get oriented for a new coding session in under 30 seconds

---

## Phase 2: Requirements (Complete)

Full Design Brief: [`docs/design-brief.md`](docs/design-brief.md)

Brief validation: 30/33 pass, 2 degraded (API surface — intentionally deferred to Architecture), 1 acceptable (bulk ops), 0 blocking. Ready for Phase 3.

### v1 Scope Summary

**Entities**: Project, Session/Handoff, Document, Relationship
**Core workflows**: Orient across projects, Resume project context, Reference docs mid-session, Search across projects, End session (create handoff)
**Key constraints**: Read-only (no code editing, no CI/CD), single-user, fully local, compact/dense UI for ultrawide side-panel use

---

## Stack Decision

Same stack as MIR — **Tauri 2 + Svelte 5 + Rust backend**. Reasons:
- We know it. No ramp-up time.
- Desktop app fits: reads local filesystem, git repos, no server needed.
- Rust backend handles git operations (libgit2), file watching, markdown parsing.
- Svelte 5 frontend with the design system we're building in TaurUI.

---

## Phase 3: UI Design

Before architecture, run the TaurUI design procedure (3A through 3G).

**Starting references to explore with `/lookbook` (during 3D-3E):**
- Navigation: Linear-style narrow sidebar (project list) + detail area
- Density: Comfortable (this is an overview tool, not a data-dense analysis tool)
- Git history: Look at Tower, GitHub Desktop, GitKraken for commit log patterns
- Markdown rendering: Obsidian, Notion, Slab for doc viewing
- Dashboard: Vercel dashboard for project grid layout
- Category files to consult: `developer-tools.md`, `productivity.md`, `knowledge-documents.md`

---

## What to Do Next

1. ~~**Phase 2** — Requirements~~ Done
2. ~~**Phase 3** — UI Design~~ Done (3A–3G complete)
3. ~~**Phase 4** — Architecture~~ Done (22 ADRs across 6 topics)
4. **Scaffold**: Initialize Tauri 2 + Svelte 5 project — structure follows from architecture
5. **Phase 5** — Implementation: TDD approach, view by view, using `/lookbook` for mid-build design questions

---

## Existing Projects to Register

Based on `~/projects/`:

| Project | Description | Active? |
|---------|-------------|---------|
| missing_invoice_reloaded | Invoice forensics tool (Tauri 2 + Svelte 5) | Yes |
| taurui | UI design knowledge base | Yes |
| taursec | Security audit knowledge base | Yes |
| taursult | Multi-model AI consultation MCP server | Yes |
| ledger | Reference project (Tauri patterns) | Reference |
| taurhaus | This project | New |
| aitx | tmux CLI wrapper | Stable |
| taurmolt | ? | Check |
| taurora | ? | Check |
| taurox | ? | Check |
