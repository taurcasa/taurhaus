# Session Handoff — Phase 5 Transition

## What Was Done This Session

### Phase 4: Architecture (Complete)

Worked through all 6 architecture topics via interactive questionnaire. Every decision was discussed with the user, alternatives evaluated, and rationale documented. Produced 22 Architecture Decision Records in `docs/phase-4-architecture.md`.

### Architecture Decisions Summary

**Storage (ADR-001 through ADR-004)**:
- Hybrid: SQLite for metadata/sessions/relationships/settings, tantivy for full-text search, filesystem as source of truth for all file content
- Data location: Tauri `app_data_dir()` — resolves to platform-appropriate path
- Session handoffs: parsed on detection, structured fields stored in SQLite, original file stays on disk
- File content: NEVER stored in database — always read live from disk
- Git state: always queried live via libgit2, cached briefly in memory only

**Data Model (ADR-006 through ADR-010)**:
- Project primary key: UUID (auto-generated on registration, path is mutable field)
- Activity state: computed on every read from `last_activity_at` + configurable thresholds (Active <7d, Recent <30d, Stale <90d, Dormant 90d+)
- Session fields: core typed fields (date, project, summary, next_steps[], open_questions[]) + extensible metadata JSON blob for everything else
- Relationships: fully automatic detection from project signals (Cargo.toml deps, CLAUDE.md refs, session mentions). Opt-out, NOT opt-in. User can dismiss false positives.
- Overview tab hero: Session/README toggle — smart default (session if <7 days old, else README), user preference remembered per project

**IPC & Rendering (ADR-011 through ADR-012)**:
- Fine-grained IPC commands (~25 total). One command per operation. Frontend calls in parallel. Tauri IPC is in-process (~0.1ms per call), so no batching needed.
- Markdown rendering in frontend with Shiki (VS Code TextMate grammars). Raw text over IPC. Pattern proven in ledger project. Backend sends raw content, never pre-rendered HTML.

**Rust Backend (ADR-013 through ADR-015)**:
- Git engine: libgit2 via `git2` crate. In-process, no CLI dependency.
- Project discovery: directory scan + auto-registration. Walk `~/projects/`, find `.git/` dirs, auto-register. File watcher detects new projects appearing.
- 8 Rust modules with strict boundaries: `commands/`, `db/`, `git/`, `fs/`, `search/`, `scanner/`, `session/`, `config/`, plus `claude_code/` (v1.1) and `models/`

**Claude Code Integration (ADR-016 through ADR-019)**:
- Auto-handoff via `SessionEnd` hook with agent type. Hook fires when session ends, receives `session_id` + `transcript_path`, spawns Haiku agent that reads transcript and writes structured handoff.
- Manual `/handoff` skill as fallback (replaces existing `/whats-next`). For mid-session checkpoints or when hook fails.
- Handoff format: markdown with YAML frontmatter (core fields) + JSON metadata sidecar (session_id, duration, tools_used, files_modified, tokens).
- Files land in `docs/sessions/session-YYYY-MM-DDTHH-MM-SS.{md,meta.json}` within the project.
- Claude Code data display (memory, teams, tasks) designed for v1.1 — module exists but no UI in v1.

**File Watching (ADR-020 through ADR-022)**:
- `notify` + `ignore` crates. Pre-filtered watches: parse .gitignore before setting up watchers, skip ignored directories entirely (saves thousands of inotify descriptors on Linux).
- If .gitignore changes → automatic watch rebuild for that project.
- .git internals (HEAD, index, refs/) watched with 2-second debounce to prevent event storms during git operations.
- Scan directories watched at depth 1 for auto-discovery of new projects.
- Events delivered to frontend via Tauri event emission (not polling).

### System Architecture

The architecture doc includes a full system overview section with:
- Component descriptions (frontend, backend, data stores, external integration)
- 5 detailed data flows (startup, file change, session import, search query, relationship detection)
- Module dependency graph
- Claude Code ↔ taurhaus boundary (filesystem only, no direct communication)

Visual infographic at `docs/system-architecture.jpg`.

### Commits (2, on main branch)
1. `e1e6e96` — Phase 4 architecture (22 ADRs + system overview + infographic)
2. `adee4fd` — Project status updates (BOOTSTRAP.md, CLAUDE.md)

---

## Current State

### Project Structure
```
taurhaus/
  BOOTSTRAP.md              — project lifecycle (Phases 1-4 complete, Phase 5 next)
  CLAUDE.md                 — design paradigms + code standards + architecture summary
  docs/
    design-brief.md         — full requirements (entities, workflows, constraints)
    phase-3b-domain.md      — domain understanding
    phase-3c-journeys.md    — user journey mapping
    phase-3d-architecture.md — information architecture (views, navigation)
    phase-3e-views.md       — view designs
    phase-3f-visual.md      — visual system (tokens, components, colors)
    phase-3g-specification.md — implementation spec (tokenized, zero design decisions)
    phase-4-architecture.md — technical architecture (22 ADRs + system overview)
    system-architecture.jpg — visual infographic
    handoff-2026-02-17.md   — previous session handoff (Phase 3 → Phase 4)
    handoff-phase-5.md      — THIS FILE
  prototype/
    src/
      App.svelte            — thin entry wrapper
      Shell.svelte          — Floating Panel layout (THE visual reference)
      app.css               — Tailwind v4 design tokens
      data/mock.js          — hardcoded mock data (shapes are approximate)
      main.js               — Vite entry point
    package.json            — Svelte 5, Tailwind v4, Vite 6
  titlebar_concept.png      — user's Excalidraw mockup
```

### Git
- 6 commits on `main`, no remote configured
- Working tree clean

### Dev Server
```bash
cd prototype && npm run dev
# Runs on http://localhost:5173
# This is the UI prototype only — no Tauri backend yet
```

---

## What's Next: Phase 5 — Implementation

### Pre-Implementation: Development Workflow Session

Before writing code, establish the development workflow:

1. **Scaffold the Tauri 2 project** — initialize the real Tauri project structure (not the prototype). The prototype's Svelte code and CSS tokens migrate into the Tauri frontend. The Rust backend is new.

2. **TDD approach** — the user explicitly wants TDD for frontend and logic-heavy backend. Define:
   - Test framework choices (Rust: built-in `#[test]` + maybe integration tests. Frontend: vitest?)
   - What gets unit tests vs integration tests vs E2E tests
   - Test-first workflow: write test → see it fail → implement → see it pass

3. **Implementation order** — which modules to build first. Likely:
   - Phase 5A: Scaffold + SQLite schema + basic project CRUD (the foundation everything else needs)
   - Phase 5B: Git module + file reader (enables the Overview and Files tabs)
   - Phase 5C: File watcher + session import (enables real-time updates)
   - Phase 5D: Search (tantivy indexing + query)
   - Phase 5E: Scanner + relationship detection (auto-discovery)
   - Phase 5F: Claude Code hook + /handoff skill (integration)
   - Phase 5G: Polish, edge cases, Settings view, First-Run wizard

4. **Acceptance criteria** — per module or per feature, what "done" looks like. Should be defined before implementation starts.

5. **Migration strategy** — how the prototype code becomes the real app. The prototype's `Shell.svelte`, `app.css`, and design tokens carry over. The mock data gets replaced with IPC calls.

### Key Design Documents to Read (for fresh context)
- `CLAUDE.md` — design paradigms, code standards, architecture summary (READ FIRST)
- `docs/phase-4-architecture.md` — System Architecture Overview section at the top gives the big picture. ADR index table gives all 22 decisions at a glance. Individual ADRs have full context.
- `docs/phase-3g-specification.md` — implementation-ready UI spec with all token values
- `docs/phase-3d-architecture.md` — view inventory, navigation model, information grouping

### User Preferences (Consolidated)

**Decision-making style**:
- Wants to understand tradeoffs deeply before deciding — present alternatives with pros/cons, not just recommendations
- Prefers opt-out over opt-in (auto-detect, auto-register, auto-handoff)
- Doesn't want manual busywork — if it can be automated, it should be
- Values real-time updates over polling (chose .git watching over interval checks)
- Prefers conversational architecture work over plan mode (plan mode burns context on exploration)

**Code preferences**:
- Production quality from day one — no "we'll fix it later"
- Explicit actionable language ("must be replaced" not "will be replaced")
- Wants logical commit splitting
- TDD approach for frontend and logic-heavy code
- Same patterns as MIR where applicable (Tauri 2, Svelte 5, libgit2)

**UI preferences**:
- Snappy — every interaction feels instant
- Visible controls (theme toggle in titlebar, not hidden)
- Real-time information is key for a tool with "snappy" as its paradigm

---

## Open Questions (Carry Forward)

- Virtual scrolling library selection for large project lists
- Tantivy index configuration (tokenizer, schema, stored fields vs source fields)
- Exact Claude Code project hash algorithm (for the `claude_code` module's `resolver.rs`)
- `.taurhausignore` file format (plain glob patterns? .gitignore syntax?)
- SQLite migration strategy (embedded SQL files vs. Rust migration crate like refinery/sqlx-migrate)
- Session handoff skill prompt template (exact wording for the SessionEnd agent hook)
- Markdown rendering library choice: marked vs unified/remark (both work with Shiki)
- justfile recipes — reference MIR's justfile for dev/release build patterns
