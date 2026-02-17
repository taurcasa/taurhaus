# Phase 4: Architecture

> Technical architecture for taurhaus. Structured as Architecture Decision Records (ADRs) grouped by topic. Each decision captures context, the choice made, alternatives considered, and consequences.

---

## System Architecture Overview

### Components

taurhaus is a Tauri 2 desktop application. One OS process runs both the Rust backend and the webview frontend.

**Frontend (Svelte 5 webview)**:
- Renders all UI (sidebar, content panels, search overlay, settings)
- Calls Rust backend via Tauri IPC (`invoke()`) for all data
- Listens to Tauri events for real-time updates from file watchers
- Renders markdown and syntax highlighting in-browser (Shiki + marked/unified)
- Never reads the filesystem directly — all data comes through the backend

**Rust backend (src-tauri/)**:
- 8 modules: `commands`, `db`, `git`, `fs`, `search`, `scanner`, `session`, `config`, plus `claude_code` (v1.1) and `models` (shared structs)
- `commands/` is the only Tauri-aware layer — thin wrappers that call into domain modules
- `db/` owns the SQLite connection — all other modules interact with it through typed query functions
- `fs/watcher` runs on a background thread, emits Tauri events when changes are detected
- `search/` owns the tantivy index — builds it, updates it, queries it

**Data stores**:
- **SQLite** (`app_data_dir()/taurhaus.db`): projects table, sessions table, relationships table, settings table. User-owned and taurhaus-derived data.
- **Tantivy index** (`app_data_dir()/search_index/`): full-text search index over all text content (docs, sessions, commit messages). Derived — can always be rebuilt from source files.
- **Filesystem** (project directories): source of truth for all file content, git state, handoff files. Read-only from taurhaus's perspective (except handoff files which are written by Claude Code hooks, not by taurhaus itself).

**External integration**:
- **Claude Code** writes handoff files via a `SessionEnd` hook agent. Files land in `docs/sessions/` within the project. taurhaus's file watcher detects and imports them. No direct communication between the two processes.

### Data Flows

**Flow 1: App Startup**
```
1. Tauri launches → Rust main.rs runs
2. Open SQLite database (create if first run)
3. Run pending migrations
4. Load settings (scan directories, thresholds, ignore patterns)
5. If first run (no projects registered):
   → Frontend shows V-06 First-Run Setup
   → User picks scan directory → backend walks tree → discovers git repos
   → User selects which to register → backend creates project records
   → Backend builds initial tantivy index (progress events → frontend)
6. If projects exist:
   → Start file watchers for all registered projects (pre-filtered by .gitignore)
   → Start scan directory watcher (depth 1, for auto-discovery)
   → Frontend requests list_projects → backend queries SQLite → returns ProjectSummary[]
   → Frontend renders sidebar
7. User selects project → frontend fires parallel IPC calls:
   get_project, get_latest_session, get_recent_commits,
   get_relationships, get_readme
   → Each section renders independently as its data arrives
```

**Flow 2: File Change → UI Update**
```
1. User edits a file in their IDE/Claude Code
2. notify crate detects the filesystem event
3. Event passes through ignore filter (skip if .gitignore'd)
4. Event type determines handler:
   a. Text file changed → update tantivy index for that file
   b. .gitignore changed → rebuild watch set for this project
   c. docs/sessions/session-*.md created → trigger session import
   d. .git/HEAD or .git/index changed → debounce 2s → run git status
5. Backend emits Tauri event to frontend:
   - project-files-changed { project_id, paths }
   - project-git-changed { project_id, branch, is_dirty }
   - session-imported { project_id, session_id }
6. Frontend reacts:
   - Sidebar updates dirty indicator / branch name
   - File tree refreshes if viewing that project
   - Session card animates in if on Overview tab
```

**Flow 3: Session Handoff Import**
```
1. Claude Code session ends
2. SessionEnd hook fires → receives session_id + transcript_path on stdin
3. Agent hook (Haiku model) spawns:
   a. Reads transcript JSON from transcript_path
   b. Extracts summary, next steps, open questions, decisions
   c. Writes docs/sessions/session-YYYY-MM-DDTHH-MM-SS.md (YAML frontmatter + markdown)
   d. Writes docs/sessions/session-YYYY-MM-DDTHH-MM-SS.meta.json (session metadata)
4. taurhaus file watcher detects new .md file in docs/sessions/
5. session/parser.rs:
   a. Reads the markdown file
   b. Parses YAML frontmatter → extracts core fields (date, summary, next_steps, open_questions)
   c. Remaining YAML keys → stored as metadata JSON blob
   d. Reads companion .meta.json if present → stores session metadata
   e. Matches project by cwd/path → links via project UUID
   f. Inserts into sessions SQLite table
   g. Adds session text to tantivy index
6. Backend emits session-imported event
7. Frontend: if viewing this project's Overview tab, session card appears with highlight animation
```

**Flow 4: Search Query**
```
1. User presses Cmd+K → search overlay opens
2. User types query → frontend debounces 150ms → calls search(query, limit: 20)
3. Backend search/query.rs:
   a. Runs tantivy query with BM25 ranking
   b. Returns SearchResult[] with: project_id, entity_type (document/session/commit),
      file_path, snippet with match positions, relevance score
4. Frontend renders results grouped by type with highlighted match terms
5. User selects result → overlay dismisses → navigates to target view
   - Document result → V-03 with file selected in tree
   - Session result → V-02 with session expanded
   - Commit result → V-02 scrolled to commit
```

**Flow 5: Relationship Auto-Detection**
```
1. Triggered during: project registration, file watcher events on key files, periodic rescan
2. scanner/relationships.rs scans each project for signals:
   a. Parse Cargo.toml → extract path dependencies → infer depends_on
   b. Parse package.json → extract local file: deps → infer depends_on
   c. Check .gitmodules → infer includes
   d. Grep CLAUDE.md for registered project names → infer references
   e. Search session content for project name mentions → infer mentioned_in_session
3. Compare detected relationships against existing relationships table
4. New detection → insert with dismissed=false, detection_source=signal_type
5. Signal disappeared (e.g., dependency removed from Cargo.toml):
   → Update last_seen_at, optionally mark as stale after N days
6. Never auto-remove manually created relationships (detection_source="manual")
```

### Module Dependency Graph

This shows which modules call which. Arrows mean "calls into" / "depends on".

```
commands/  →  db/
commands/  →  git/
commands/  →  fs/
commands/  →  search/
commands/  →  scanner/
commands/  →  session/
commands/  →  config/

scanner/   →  db/        (register discovered projects)
scanner/   →  git/       (check if directory is a git repo)
scanner/   →  fs/        (walk directories)

session/   →  db/        (store parsed sessions)
session/   →  search/    (index session content)

fs/watcher →  search/    (trigger re-index on file change)
fs/watcher →  session/   (trigger import on new handoff file)
fs/watcher →  git/       (trigger status refresh on .git changes)

search/    →  db/        (read project/session metadata for result context)

models/    ←  (used by all modules — shared structs, no logic)
```

**Strict boundaries**:
- `commands/` is the ONLY module that imports Tauri types
- `db/` is the ONLY module that opens SQLite connections
- `git/` is the ONLY module that uses libgit2
- `search/` is the ONLY module that uses tantivy
- `fs/watcher` is the ONLY module that uses notify

### Boundary: Claude Code ↔ taurhaus

There is NO direct communication between Claude Code and taurhaus. The boundary is the filesystem.

```
Claude Code process                    taurhaus process
─────────────────                     ────────────────
SessionEnd hook fires          →      (nothing — taurhaus doesn't know)
Agent writes handoff .md file  →      File watcher detects new file
Agent writes .meta.json        →      File watcher detects new file
                                      session/parser imports both files
                                      SQLite + tantivy updated
                                      Tauri event emitted to frontend
                                      UI updates
```

The `/handoff` skill (manual fallback) follows the same path — it writes files, taurhaus watches for them. The mechanism is identical whether the handoff was auto-created by the SessionEnd hook or manually created by the skill.

**Implications**:
- taurhaus doesn't need to be running when Claude Code writes a handoff. The file sits on disk. Next time taurhaus starts, it scans for unimported handoff files and imports them.
- If Claude Code adds new fields to the handoff format, taurhaus's parser handles them gracefully: core fields are typed, everything else goes into the extensible metadata JSON blob.
- No coordination, no ports, no sockets, no shared memory. Just files.

---

## ADR Index

| # | Topic | Decision | Key |
|---|-------|----------|-----|
| 001 | Storage | Hybrid: SQLite + filesystem | SQLite for metadata, filesystem for content |
| 002 | Search | Tantivy for FTS | Separate from SQLite, purpose-built |
| 003 | Data location | `app_data_dir()` | Platform-appropriate via Tauri API |
| 004 | File access | Category-based | SQLite owns taurhaus data, filesystem owns existing content |
| 005 | Platform | Windows first | Linux for dev, Windows release for daily use |
| 006 | Hero content | Session/README toggle | Smart default, user override per project |
| 007 | Project PK | UUID | Independent of path, survives moves |
| 008 | Activity state | Computed on read | From `last_activity_at` + configurable thresholds |
| 009 | Session fields | Core + extensible metadata | Typed core fields, JSON blob for the rest |
| 010 | Relationships | Auto-detected, opt-out | Scan Cargo.toml, CLAUDE.md, sessions for signals |
| 011 | IPC style | Fine-grained (~25 commands) | One command per operation, parallel frontend calls |
| 012 | Markdown | Frontend + Shiki | Raw text over IPC, rendered in browser |
| 013 | Git engine | libgit2 (git2 crate) | In-process, no CLI dependency |
| 014 | Discovery | Scan + auto-register | Walk dirs, watch for new projects |
| 015 | Modules | 8 backend modules | Clear boundaries, independently testable |
| 016 | Auto-handoff | SessionEnd hook + agent | Automatic on every session end |
| 017 | Manual handoff | /handoff skill | Fallback for crashes, mid-session checkpoints |
| 018 | Handoff format | MD + YAML frontmatter + JSON sidecar | Two files per session |
| 019 | CC data | Designed for v1.1 | Module exists, no UI |
| 020 | File watcher | notify + ignore, pre-filtered | .gitignore-aware watch setup |
| 021 | Scan watcher | Depth-1 on scan dirs | Auto-discover new projects |
| 022 | Event delivery | Tauri events | Backend pushes to frontend, no polling |

---

## Topic 1: Storage Strategy

### ADR-001: Hybrid Storage (SQLite + Filesystem)

**Context**: taurhaus needs to store user metadata (project registry, sessions, relationships, settings), provide full-text search across all content, and display file content from project directories. Three approaches were evaluated.

**Decision**: Hybrid — SQLite for metadata and structured data, filesystem as source of truth for all file content and git state.

**Alternatives considered**:

| Approach | Pros | Cons |
|----------|------|------|
| **SQLite-only** | Simpler queries, single system | Duplicates filesystem data, constant sync needed |
| **Filesystem-only** | Zero drift, git-trackable config | Weak for search/queries, listing N projects = N file reads |
| **Embedded KV (redb)** | Pure Rust, no FFI, fast point reads | No FTS, no query language, manual secondary indexes, building a mini query engine |

**Rationale**: taurhaus needs structured queries ("list projects sorted by activity", "all sessions for project X") and full-text search. SQLite provides both query capability and FTS5. Filesystem stays canonical for content that already exists on disk. The KV approach (redb + tantivy) was evaluated for Rust-native purity but would require hand-coding all query logic — significant effort for no measurable performance gain at our scale (50-100 projects).

**Consequences**:
- SQLite via `rusqlite` crate (or `sqlx` for async)
- File content is never stored in the database — always read live from disk
- Git state is always queried live (libgit2 or CLI), not cached in DB
- Schema migrations needed as the data model evolves

---

### ADR-002: Search Engine — Tantivy

**Context**: Full-text search across all project content (docs, sessions, commits) is a v1 requirement. Need sub-200ms results as the user types.

**Decision**: Tantivy for full-text search. SQLite for structured queries.

**Alternatives considered**:

| Approach | Pros | Cons |
|----------|------|------|
| **SQLite FTS5 only** | One dependency, simpler | Slower for large indexes, less flexible ranking, no stemming/fuzzy by default |
| **Tantivy only** | Fast, Rust-native, rich features | Can't replace SQLite for structured queries |
| **SQLite FTS5 + tantivy** | Overkill | Two search systems to maintain |

**Rationale**: Tantivy is purpose-built for full-text search — fast indexing, BM25 ranking, stemming, fuzzy matching. SQLite FTS5 is adequate but tantivy scales better and provides a richer query experience. Since we already have SQLite for structured data, tantivy handles the search domain cleanly.

**Consequences**:
- Tantivy index stored alongside the SQLite database in `app_data_dir()`
- Index is derived — can always be rebuilt from source files
- Index updates triggered by file watcher events
- Two data stores to manage (SQLite + tantivy index directory)

---

### ADR-003: Data Location — Tauri app_data_dir()

**Context**: taurhaus needs a location for its SQLite database, tantivy index, and configuration. Must work on both Windows (primary target) and Linux/WSL2 (development).

**Decision**: Use Tauri's `app_data_dir()` API, which resolves to the platform-appropriate location.

| Platform | Resolves to |
|----------|-------------|
| Windows | `C:\Users\<user>\AppData\Roaming\taurhaus\` |
| Linux | `~/.local/share/taurhaus/` |

**Alternatives considered**:
- `~/.taurhaus/` — simple but not platform-standard
- Per-project `.taurhaus/` directories — fragments the database, breaks cross-project search

**Rationale**: Tauri provides this API specifically for cross-platform data storage. Using it means zero platform-specific path logic in our code. Project directories stay clean.

**Consequences**:
- All taurhaus-owned data lives in one directory
- Easy to back up, reset, or migrate
- Tauri's `path` plugin required in the frontend for any path resolution

---

### ADR-004: File Access Philosophy

**Context**: taurhaus displays many types of content — session handoffs, READMEs, source code, docs, git state. Need a consistent approach to how each type is accessed.

**Decision**: Content is categorized by ownership. taurhaus-owned data lives in SQLite. Everything else is read live from the filesystem.

| Category | Examples | Access | Storage |
|----------|----------|--------|---------|
| **User metadata** | Project name, tags, description, relationships | Read/write | SQLite |
| **Structured imports** | Session handoffs | Parsed on detection, structured fields in SQLite | SQLite (parsed) + filesystem (original) |
| **Live content** | README.md, source code, docs, images | Read on demand | Filesystem only |
| **Git state** | Commits, branches, dirty status | Queried live | In-memory (short-lived cache) |
| **Search index** | FTS over all text content | Derived, rebuilt from sources | Tantivy index |
| **Settings** | Scan dirs, thresholds, ignore patterns | Read/write | SQLite |

**Rationale**: This avoids the "cache everything" trap. Files on disk are the source of truth — taurhaus reads them, never copies them into a database. Session handoffs are the one exception because their structured fields (summary, next_steps, open_questions) need to be queryable and searchable without re-parsing YAML on every access.

**Consequences**:
- File content display always reflects the current state on disk
- No sync/staleness issues for documents
- Session handoffs are imported once, re-imported if the file changes (file watcher detects modification)
- Git operations may have latency (10-100ms) — UI should handle this gracefully

---

### ADR-005: Target Platform — Windows First, Linux for Development

**Context**: Development happens in WSL2 (Linux), but the GUI experience on WSL2 via WSLg is subpar — window management issues, occasional crashes, debug builds feel sluggish. MIR had the same experience: Windows release builds felt significantly better.

**Decision**: Target Windows as the primary runtime. Use Linux/WSL2 for development and testing (debug builds, `cargo check`, etc.). Windows release builds for actual use.

**Rationale**: Tauri apps render via the system webview — on Windows this is Edge/WebView2 (polished), on WSLg it's whatever X11/Wayland compositor is available (less polished). The projects live in WSL2 filesystem, accessible from Windows via `\\wsl$\` paths. Cross-boundary file access adds ~2-5x latency for random reads, but for taurhaus access patterns (read a markdown file, run git commands), this is milliseconds.

**Consequences**:
- `justfile` recipes for both dev (Linux) and release (Windows) builds, following MIR's pattern
- File paths must use Tauri's path APIs, never hardcoded Unix paths
- Git operations from Windows accessing WSL2 repos need testing — may need `wslpath` translation or WSL2 git
- Development workflow: edit code in WSL2, test with `cargo tauri dev` on Linux, periodically build Windows release

---

## Topic 2: Data Model

### ADR-006: Overview Tab — Session/README Toggle

**Context**: The Phase 3 spec puts the latest session handoff as the hero content on the Overview tab. But READMEs are the natural "landing page" for a project and are more reliably present (auto-generated, not dependent on invoking a skill). Sessions are high-value when recent but may not exist or may be stale.

**Decision**: The Overview tab has a small toggle (segmented control) at the top of the hero area: **Session | README**. Switches which content is displayed prominently.

**Behavior**:
- If only one exists, show that one — no toggle needed
- If both exist, show the toggle with smart default: latest session if <7 days old, otherwise README
- User's toggle choice is remembered per project (stored in SQLite as a preference)
- The non-active content is one click away via the toggle

**Alternatives considered**:
- Stacked sections (both visible, scroll between) — too much content competing for attention
- Side-by-side at wide viewports — complex responsive layout, README can be long

**Consequences**:
- Backend must report whether a README exists for each project
- README content served via the same live-file-read path as the document viewer
- Per-project preference stored in the projects table (nullable `hero_preference` column)
- UI component: segmented control, similar to the Overview/Files tab bar but smaller

---

### ADR-007: Project Primary Key — UUID

**Context**: Projects need a stable identity for linking sessions, relationships, and preferences. Options: filesystem path, UUID, or slug derived from directory name.

**Decision**: Auto-generated UUID, assigned at project registration.

**Alternatives considered**:
- **Path as PK**: Simple and debuggable, but breaks if a project moves on disk — all linked data (sessions, relationships) becomes orphaned.
- **Slug**: Human-readable (`taurhaus`, `missing-invoice-reloaded`), but collision risk if two projects share a directory name in different paths.
- **Git root commit SHA**: Git repos don't have a built-in UUID. Root commit is shared across clones/forks, so not unique.

**Consequences**:
- Path stored as a mutable `path` field — updatable if the project moves
- All foreign keys (sessions, relationships, preferences) reference the UUID
- UUID is opaque to the user — never displayed in the UI, only used internally

---

### ADR-008: Activity State — Computed on Read

**Context**: Projects display an activity state (Active/Recent/Stale/Dormant) in the sidebar and project header. This state is derived from how recently the project had git activity.

**Decision**: Computed on every read from `last_activity_at` timestamp using configurable thresholds from Settings.

**Thresholds** (defaults, configurable in Settings V-05):
- **Active**: last activity within 7 days
- **Recent**: 7–30 days
- **Stale**: 30–90 days
- **Dormant**: 90+ days

**Rationale**: At 50-100 projects, computing a date comparison per project on each list render is negligible (<1ms total). Storing the state would introduce sync complexity — the state would need updating every time a threshold boundary is crossed, which happens silently with the passage of time.

**Consequences**:
- `last_activity_at` timestamp stored in the projects table, updated on git events
- Activity state is a derived property in the Rust struct, not a DB column
- Threshold values stored in the settings table
- Frontend receives the computed state string, not the raw timestamp (backend computes it)

---

### ADR-009: Session Handoff Fields — Core + Extensible Metadata

**Context**: Session handoffs are structured files produced by a Claude Code skill. taurhaus parses and imports them. The schema must cover the UI needs (V-02 session card) while being forward-compatible.

**Decision**: Core typed fields plus a freeform metadata block.

**Core fields** (typed, queryable, indexed):
- `date`: ISO 8601 date
- `project`: project name or path (matched to registered project by UUID)
- `summary`: free text
- `next_steps`: string array
- `open_questions`: string array

**Extensible metadata** (stored as JSON blob, searchable but not individually queryable):
- `decisions_made`: array of strings
- `files_changed`: array of strings
- `branch`: string
- `commit_range`: string (e.g., `abc1234..def5678`)
- `claude_session_id`: string (if capturable)
- Any other key-value pairs the skill produces

**Rationale**: Core fields cover the V-02 session card layout. The metadata block allows the skill to evolve without requiring database migrations. New fields can be added to the skill and immediately stored/searched — the UI just doesn't render them until we add display logic.

**Consequences**:
- SQLite schema: typed columns for core fields, a `metadata JSON` column for the rest
- Tantivy indexes all text content (summary + next_steps + open_questions + metadata values)
- The Claude Code skill template must produce files matching this schema
- Session import logic: parse YAML frontmatter → extract core fields → dump remaining keys into metadata blob

---

### ADR-010: Relationships — Fully Automatic Detection (Opt-Out)

**Context**: The design brief defined relationships as user-created typed links between projects. But manually creating relationships has low value — if you know the relationship well enough to create it, you've already internalized the knowledge. The real value is in surfacing relationships you've forgotten or don't realize exist.

**Decision**: taurhaus auto-detects relationships from project signals. Detected relationships are shown automatically. Users can dismiss false positives (opt-out), not confirm true positives (opt-in).

**Detection signals**:

| Signal | Detection method | Inferred type |
|--------|-----------------|---------------|
| Cargo.toml path dependencies | Parse `[dependencies]` for `path = "../project"` | `depends_on` |
| package.json local deps | `"file:../project"` in dependencies | `depends_on` |
| Git submodules | `.gitmodules` entries | `includes` |
| Cargo/npm workspace members | Workspace config files | `workspace_sibling` |
| CLAUDE.md references | Grep for registered project names/paths | `references` |
| Session handoff mentions | Search handoff text for registered project names | `mentioned_in_session` |

**Relationship data model**:
- `id`: UUID
- `source_project_id`: UUID (FK to projects)
- `target_project_id`: UUID (FK to projects)
- `relationship_type`: enum string (from detection signal)
- `detection_source`: how it was detected (e.g., "cargo_toml", "claude_md", "session_mention")
- `dismissed`: boolean (user opted out of seeing this one)
- `first_detected_at`: timestamp
- `last_seen_at`: timestamp (updated on each scan — if a dependency is removed from Cargo.toml, relationship can be auto-removed after it's no longer detected)

**Manual creation**: Still available in the UI for relationships that can't be auto-detected (e.g., "this project was inspired by that one"). Manual relationships have `detection_source = "manual"` and are never auto-removed.

**Consequences**:
- Relationship detection runs during project scanning (initial + file watcher updates)
- No confirmation UI needed — relationships just appear
- Dismiss action stores `dismissed = true` — the relationship is hidden but remembered (won't re-suggest)
- If a signal disappears (dependency removed), the relationship can be auto-removed or flagged as stale
- Detection logic is extensible — new signal types can be added without schema changes

---

## Topic 3: Tauri IPC Command Surface

### ADR-011: Fine-Grained IPC Commands

**Context**: Tauri IPC is in-process message passing (~0.1ms per round-trip), not HTTP. The question is whether commands should match individual operations or be composed into view-shaped responses.

**Decision**: Fine-grained — one command per operation. Frontend composes views by calling multiple commands in parallel.

**Alternatives considered**:
- **View-oriented**: Single `get_project_overview` returning everything the Overview tab needs. Fewer calls but couples backend to UI layout. Can't render sections independently.
- **Fine-grained + batch**: Fine-grained by default plus a `batch` command for atomic consistency. Unnecessary — taurhaus doesn't need cross-query atomicity.

**Rationale**:
- Tauri IPC overhead is negligible (4 parallel calls ≈ 0.4ms)
- Each section renders independently with its own loading state (skeleton → content)
- Commands are testable in isolation
- Commands are reusable across views (e.g., `get_recent_commits` used in both Overview and search results)
- Svelte 5 `$effect` handles parallel async naturally

**IPC Command Inventory**:

| Command | Input | Output | Used by |
|---------|-------|--------|---------|
| **Projects** | | | |
| `list_projects` | sort_by, filter | `ProjectSummary[]` | V-01 sidebar |
| `get_project` | project_id | `ProjectDetail` | V-02 header |
| `register_project` | path | `ProjectDetail` | Registration modal |
| `register_projects_batch` | paths[] | `ProjectDetail[]` | First-run wizard |
| `update_project` | project_id, fields | `ProjectDetail` | Edit metadata |
| `remove_project` | project_id | void | Remove (unregister) |
| `scan_directory` | path | `DiscoveredProject[]` | First-run, rescan |
| **Sessions** | | | |
| `get_latest_session` | project_id | `Session \| null` | V-02 session card |
| `list_sessions` | project_id, limit, offset | `SessionSummary[]` | V-02 session history |
| `get_session` | session_id | `SessionDetail` | Expanded session |
| **Git** | | | |
| `get_recent_commits` | project_id, limit | `Commit[]` | V-02 recent activity |
| `get_all_commits` | project_id, limit, offset | `Commit[]` | Expanded commit history |
| `get_git_status` | project_id | `GitStatus` | Sidebar dirty indicator |
| **Files** | | | |
| `get_file_tree` | project_id | `FileTreeNode[]` | V-03 file tree |
| `read_file` | project_id, relative_path | `FileContent` | V-03 content area |
| `get_readme` | project_id | `FileContent \| null` | V-02 README toggle |
| **Search** | | | |
| `search` | query, limit | `SearchResult[]` | V-04 command palette |
| **Relationships** | | | |
| `get_relationships` | project_id | `Relationship[]` | V-02 relationships section |
| `dismiss_relationship` | relationship_id | void | Dismiss auto-detected |
| `create_relationship` | source_id, target_id, type, description | `Relationship` | Manual creation |
| `remove_relationship` | relationship_id | void | Remove manual relationship |
| **Settings** | | | |
| `get_settings` | — | `Settings` | V-05 |
| `update_settings` | fields | `Settings` | V-05 |
| **Index** | | | |
| `get_index_status` | — | `IndexStatus` | V-05 index section |
| `rebuild_index` | — | void (streams progress) | V-05 rebuild button |

**Consequences**:
- ~25 commands to implement
- Each command maps to a Rust function annotated with `#[tauri::command]`
- Frontend creates thin async wrappers per command (type-safe `invoke` calls)
- Pagination via `limit` + `offset` where volume can be large (commits, sessions)

---

### ADR-012: Markdown Rendering — Frontend with Shiki

**Context**: taurhaus needs to render markdown documents and syntax-highlight source code. Options: Rust backend (comrak + syntect) or JavaScript frontend (marked/unified + Shiki).

**Decision**: Frontend rendering. Raw markdown/source sent over IPC, rendered in the browser with Shiki for syntax highlighting.

**Alternatives considered**:
- **Rust backend (comrak + syntect)**: Pre-rendered HTML sent over IPC. Zero UI thread cost. But: syntect uses Sublime grammars (fewer themes, no VS Code compat), and interactive features (copy buttons, collapsible blocks) require frontend post-processing anyway.

**Rationale**:
- Shiki uses VS Code's TextMate grammars — exact same highlighting quality as VS Code
- Light/dark theme support comes free (Shiki themes map to our theme toggle)
- Full styling control — interactive elements like copy buttons, collapsible sections, internal link interception
- Pattern proven in ledger (same Tauri + frontend rendering stack)
- Performance manageable: lazy-render for files >1000 lines, or offload parsing to a web worker

**Consequences**:
- `read_file` IPC command returns raw text content, not rendered HTML
- Frontend needs a markdown rendering library (marked or unified/remark) + Shiki
- Shiki loaded once at app startup (~100KB WASM), grammars loaded on demand
- Large file performance: set a threshold (e.g., >5000 lines) where we show a warning or truncate with "show full file" button

---

## Topic 4: Rust Backend Modules

### ADR-013: Git Engine — libgit2 (git2 crate)

**Context**: taurhaus needs to read git state: commit history, branches, working tree status, diffs. Options: libgit2 Rust bindings or shelling out to the git CLI.

**Decision**: libgit2 via the `git2` crate.

**Alternatives considered**:
- **Git CLI**: Full feature parity with git, but process spawn overhead per call, platform-dependent output parsing (Windows git vs WSL git), and requires git installed.
- **Hybrid**: libgit2 for common ops, CLI for edge cases. Unnecessary complexity for our read-only use case.

**Rationale**: libgit2 covers everything taurhaus needs: log, status, branch info, HEAD resolution, tree walking. No external dependency. In-process, fast. MIR already uses it.

**Consequences**:
- `git2` crate added to Cargo.toml
- All git operations are in-process, no subprocess spawning
- Git-related errors (not a repo, corrupted, permissions) handled via typed error enums
- For Windows accessing WSL2 repos via `\\wsl$\` paths: git2 should work since it reads the `.git` directory directly, but needs testing

---

### ADR-014: Project Discovery — Directory Scan + Auto-Registration

**Context**: Users have 30-50+ projects in `~/projects/`. Registering each manually is tedious. Need an efficient discovery mechanism.

**Decision**: Scan + watch. Initial directory scan finds all git repos. File watcher detects new repos appearing and auto-registers them.

**Scan behavior**:
- Walk subdirectories of configured scan directories (default: `~/projects/`)
- Identify git repos by presence of `.git/` directory
- Depth limit: 2 levels (finds `~/projects/foo/` but not `~/projects/foo/bar/baz/`)
- Ignore patterns: skip `node_modules`, `.git`, `target`, `dist` directories during walk
- First-run: present discovered repos in selection UI (checkbox list per V-06 spec)
- Rescan: discover new repos not yet registered, present for registration

**Auto-registration** (post first-run):
- File watcher on scan directories detects new directories
- If new directory contains `.git/`, auto-register it
- Follows opt-out pattern from ADR-010 — appears automatically, user can remove

**Consequences**:
- Scanner module needs efficient directory walking (ignore patterns, depth limits)
- Scanning 50 projects with depth-2 walk takes <1s on local filesystem
- Cross-boundary scanning (Windows → WSL2 `\\wsl$\`) may be slower — needs testing, possibly async with progress
- File watcher on scan directories is separate from per-project file watchers

---

### ADR-015: Rust Module Structure

**Decision**: Seven backend modules, each with a clear boundary.

```
src-tauri/src/
├── main.rs              — Tauri app setup, command registration
├── commands/            — Tauri IPC command handlers (thin wrappers)
│   ├── mod.rs
│   ├── projects.rs
│   ├── sessions.rs
│   ├── git.rs
│   ├── files.rs
│   ├── search.rs
│   ├── relationships.rs
│   └── settings.rs
├── db/                  — SQLite schema, migrations, queries
│   ├── mod.rs
│   ├── schema.rs        — Table definitions
│   ├── migrations/      — SQL migration files
│   └── queries.rs       — Typed query functions
├── git/                 — Git operations via libgit2
│   ├── mod.rs
│   ├── commits.rs
│   ├── status.rs
│   └── branches.rs
├── fs/                  — File system operations
│   ├── mod.rs
│   ├── tree.rs          — File tree building
│   ├── reader.rs        — File content reading
│   └── watcher.rs       — File watching (notify crate)
├── search/              — Tantivy full-text search
│   ├── mod.rs
│   ├── indexer.rs        — Index building and updates
│   └── query.rs          — Search query execution
├── scanner/             — Project discovery
│   ├── mod.rs
│   ├── discover.rs       — Directory walking, git repo detection
│   └── relationships.rs  — Auto-detect relationships from project signals
├── session/             — Handoff file parsing and import
│   ├── mod.rs
│   └── parser.rs         — YAML frontmatter + markdown parsing
├── config/              — App settings
│   └── mod.rs
└── models/              — Shared Rust structs (Project, Session, etc.)
    └── mod.rs
```

**Boundaries**:
- `commands/` — only Tauri-specific code. Calls into other modules. Never contains business logic.
- `db/` — only SQLite. No git, no filesystem.
- `git/` — only libgit2. No database access.
- `models/` — shared structs with `serde::Serialize` for IPC and `FromRow` for SQLite. No logic.
- Each module is independently testable.

**Key crates**:

| Crate | Purpose |
|-------|---------|
| `rusqlite` | SQLite with bundled libsqlite3 |
| `git2` | libgit2 bindings |
| `notify` | Cross-platform file watching |
| `tantivy` | Full-text search engine |
| `serde` / `serde_json` | Serialization for IPC and JSON metadata |
| `serde_yaml` | YAML frontmatter parsing for handoff files |
| `uuid` | UUID generation for primary keys |
| `chrono` | Date/time handling |
| `thiserror` | Typed error enums |

**Consequences**:
- Clean separation of concerns — each module can be developed and tested independently
- The `commands/` layer is the only Tauri-aware code — everything else is a plain Rust library
- Module boundaries align with the IPC command groups

---

## Topic 5: Claude Code Integration

### ADR-016: Auto-Handoff via SessionEnd Hook

**Context**: Session handoffs are the primary mechanism for context continuity between Claude Code sessions. Relying on the user to manually invoke a skill is unreliable — sessions crash, users forget, the skill invocation interrupts flow.

**Decision**: Automatic handoff creation via a Claude Code `SessionEnd` hook. The hook fires when a session terminates and spawns an agent that reads the transcript and writes a structured handoff file.

**Mechanism**:

```
Session ends (user exits, /exit, /clear, crash)
    → SessionEnd hook fires
    → Receives on stdin: { session_id, transcript_path, cwd, reason }
    → Agent hook (type: "agent") spawns
    → Agent reads transcript via transcript_path
    → Agent writes two files to docs/sessions/:
        1. session-2026-02-17T14-30-45.md    (handoff with YAML frontmatter)
        2. session-2026-02-17T14-30-45.meta.json  (session metadata)
    → taurhaus file watcher detects new files
    → Imports handoff into SQLite, stores metadata
```

**Hook configuration** (in `.claude/settings.json` or project `.claude/settings.json`):
```json
{
  "hooks": {
    "SessionEnd": [
      {
        "hooks": [
          {
            "type": "agent",
            "prompt": "Read the session transcript and create a structured handoff file. [template follows]",
            "model": "claude-haiku-4-5-20251001",
            "timeout": 60
          }
        ]
      }
    ]
  }
}
```

**Agent hook details**:
- Model: Haiku (fast, cheap — this runs on every session end)
- The agent reads the transcript, extracts core fields, writes the handoff file
- Timeout: 60s (generous — most handoffs take <15s)
- The agent has access to Read, Glob, Grep tools — can read the transcript file

**SessionEnd hook receives**:
- `session_id` — UUID of the ending session
- `transcript_path` — path to the conversation JSON on disk
- `cwd` — project working directory
- `reason` — why the session ended (`clear`, `logout`, `prompt_input_exit`, `other`)

**Consequences**:
- Handoffs are created automatically on every session end
- No user action required — fire and forget
- Costs ~$0.01-0.02 per handoff (Haiku processing the transcript)
- If the hook fails (timeout, error), the session just ends without a handoff — no user impact
- The `/handoff` skill exists as a manual fallback (see ADR-017)

---

### ADR-017: Manual /handoff Skill (Fallback)

**Context**: The SessionEnd hook is the primary handoff mechanism, but edge cases exist: sessions that crash before SessionEnd fires, mid-session checkpoints, debugging the handoff format.

**Decision**: The `/handoff` skill exists as a manual fallback, replacing the current `/whats-next` skill.

**Behavior**:
- Invoked explicitly by the user: `/handoff`
- Reads the current conversation context (not the transcript file — the skill has access to the conversation directly)
- Writes the same two files as the SessionEnd hook (handoff markdown + metadata JSON sidecar)
- Can be invoked mid-session for checkpoints — files are named with the current timestamp, multiple per session are fine

**Consequences**:
- Both auto (SessionEnd hook) and manual (/handoff skill) produce identical file formats
- taurhaus doesn't care how the file was created — it just watches for new files and imports them
- `/whats-next` is replaced by `/handoff` globally

---

### ADR-018: Handoff File Format — Markdown + YAML Frontmatter + JSON Sidecar

**Context**: The handoff file format is the contract between Claude Code and taurhaus. Must be structured enough for machine parsing, readable enough for humans.

**Decision**: Two files per session:

**1. Handoff markdown** (`session-2026-02-17T14-30-45.md`):
```markdown
---
date: 2026-02-17T14:30:45Z
project: taurhaus
session_id: abc-123-def
summary: >
  Completed Phase 4 architecture decisions. Defined storage strategy
  (SQLite + tantivy), data model, IPC commands, and Claude Code integration.
next_steps:
  - Scaffold Tauri 2 project
  - Implement SQLite schema and migrations
  - Build project scanner module
open_questions:
  - Virtual scrolling library for large project lists
  - Tantivy vs SQLite FTS5 performance comparison at scale
metadata:
  decisions_made:
    - Hybrid storage (SQLite + tantivy)
    - UUID primary keys
    - Auto-detected relationships
  branch: main
  commit_range: d7d869b..HEAD
---

## Session Notes

[Optional free-text content written by the agent or user]
```

**2. Metadata JSON sidecar** (`session-2026-02-17T14-30-45.meta.json`):
```json
{
  "session_id": "abc-123-def",
  "started_at": "2026-02-17T12:00:00Z",
  "ended_at": "2026-02-17T14:30:45Z",
  "duration_minutes": 150,
  "exit_reason": "prompt_input_exit",
  "model": "claude-opus-4-6",
  "tools_used": {
    "Edit": 23,
    "Read": 45,
    "Bash": 12,
    "Write": 5
  },
  "files_modified": [
    "docs/phase-4-architecture.md",
    "CLAUDE.md"
  ],
  "tokens": {
    "input": 245000,
    "output": 38000
  }
}
```

**File location**: `docs/sessions/` within the project directory. Created by the hook/skill. Watched by taurhaus.

**Naming convention**: `session-YYYY-MM-DDTHH-MM-SS.{md,meta.json}` — includes seconds for uniqueness when multiple sessions occur on the same day.

**Consequences**:
- Handoff files are git-trackable (they live in the project)
- Human-readable (markdown with YAML frontmatter is widely supported)
- Machine-parseable (serde_yaml for frontmatter, serde_json for sidecar)
- The metadata sidecar avoids taurhaus needing to parse raw Claude Code transcripts

---

### ADR-019: Claude Code Data — Design for v1.1, Build Later

**Context**: Claude Code stores valuable data on the filesystem: auto-memory files (`~/.claude/projects/<hash>/memory/`), team configs (`~/.claude/teams/`), task lists (`~/.claude/tasks/`). This data would enrich taurhaus's project view.

**Decision**: The architecture includes a `claude_code` module in the Rust backend that knows how to locate and parse these files. No UI in v1 — the module exists but isn't wired to IPC commands yet.

**Data taurhaus could surface in v1.1**:

| Data | Source | Value |
|------|--------|-------|
| Claude's memory | `~/.claude/projects/<hash>/memory/MEMORY.md` | What Claude "remembers" about this project |
| Topic memories | `~/.claude/projects/<hash>/memory/*.md` | Detailed notes on debugging, patterns, architecture |
| Active teams | `~/.claude/teams/{name}/config.json` | Team members, roles for collaborative sessions |
| Team tasks | `~/.claude/tasks/{name}/` | Task breakdown and status |

**Module structure** (added to ADR-015):
```
├── claude_code/         — Claude Code filesystem data (v1.1 UI)
│   ├── mod.rs
│   ├── memory.rs        — Parse auto-memory files
│   ├── teams.rs         — Parse team configs and task lists
│   └── resolver.rs      — Map project paths to Claude Code project hashes
```

**Key challenge**: Claude Code uses a hash of the project path as the directory name under `~/.claude/projects/`. taurhaus needs to resolve project paths → Claude Code hashes. This is a straightforward hash computation but needs to match Claude Code's internal algorithm.

**Consequences**:
- Module exists from v1 but has no IPC commands
- When we build v1.1 UI, we add IPC commands that call into this module
- No dependency on Claude Code's internal formats at v1 launch
- If Claude Code changes its filesystem structure, only this module needs updating

---

## Topic 6: File Watching Architecture

### ADR-020: File Watcher — notify Crate with Pre-Filtered Watches

**Context**: taurhaus needs to detect changes across 30-50+ project directories: new handoff files, file modifications (for search index updates), git state changes (branch, dirty status), and new project directories appearing.

**Decision**: `notify` crate with pre-filtered watch setup using the `ignore` crate for .gitignore parsing. Platform-appropriate behavior for efficiency.

**Watch setup per project**:
1. Parse `.gitignore` + `.taurhausignore` using the `ignore` crate
2. Walk the project tree, skipping ignored directories entirely
3. **Linux (inotify)**: Set individual watches on each non-ignored directory (`NonRecursive` mode per directory). Saves thousands of watch descriptors by skipping `node_modules/`, `target/`, `dist/`, etc.
4. **Windows (ReadDirectoryChangesW)**: Set a single recursive watch on the project root. Filter events in userspace against .gitignore patterns. Windows handles recursion efficiently with one handle per root.
5. Additionally watch `.git/HEAD`, `.git/index`, `.git/refs/heads/` for git state changes

**Watch descriptor budget** (Linux):
- Typical project after .gitignore filtering: ~50-200 directories
- 50 projects × ~100 dirs = ~5,000 watches
- Default inotify limit: 8,192 — comfortably within range
- Without filtering: node_modules alone can add 5,000+ directories per project

**Event handling logic**:
```
On file event:
  if path == ".gitignore" or path == ".taurhausignore":
    → Rebuild watch set for this project (<100ms)
    → Re-parse ignore patterns, diff current watches, add/remove as needed
  elif is_git_internal(path):  // .git/HEAD, .git/index, .git/refs/
    → Debounce 2 seconds → then run git status check once
    → Prevents event storms during git commit/rebase/merge
  elif path matches "docs/sessions/session-*.md":
    → New handoff file → trigger session import
  elif is_text_file(path):
    → File changed → update tantivy search index for this file
  elif is_directory_created(event):
    → Check if new dir should be watched (not ignored) → add watch if so
```

**Consequences**:
- Efficient resource usage — only watch what matters
- .gitignore changes trigger automatic watch rebuilds — always in sync
- Git state updates are real-time (2s debounce) without event storms
- Platform differences handled internally — the rest of the codebase doesn't know or care
- The `ignore` crate is from the ripgrep project (BurntSushi) — fast, well-tested, handles nested .gitignore files correctly

**Key crates**:

| Crate | Purpose |
|-------|---------|
| `notify` | Cross-platform file system events |
| `notify-debouncer-full` | Event debouncing (for .git internals) |
| `ignore` | .gitignore parsing and path matching (from ripgrep) |

---

### ADR-021: Scan Directory Watching — Auto-Discover New Projects

**Context**: Beyond per-project file watching, taurhaus watches the configured scan directories (default: `~/projects/`) for new project directories appearing.

**Decision**: Watch scan directories at depth 1. When a new directory appears, check for `.git/` — if present, auto-register the project (per ADR-010 opt-out pattern and ADR-014 scan + watch).

**Mechanism**:
- Watch scan directories with `notify` (depth 1 only — not recursive into projects)
- On directory CREATE event at depth 1: check for `.git/` presence
- If `.git/` found: auto-register project, set up per-project file watchers
- If no `.git/`: ignore (might be a temp directory, build artifact, etc.)

**Consequences**:
- New projects appear in the sidebar automatically
- Follows the opt-out pattern — user can remove if unwanted
- Minimal overhead — one shallow watch per scan directory

---

### ADR-022: Event Delivery to Frontend — Tauri Events

**Context**: File watcher detects changes in the Rust backend. The frontend needs to react: update the sidebar, refresh file trees, show new sessions, update git status indicators.

**Decision**: Tauri event emission from backend to frontend. The frontend subscribes to typed events.

**Event types**:

| Event | Payload | Frontend reaction |
|-------|---------|-------------------|
| `project-git-changed` | `{ project_id, branch, is_dirty }` | Update sidebar indicators |
| `project-files-changed` | `{ project_id, paths[] }` | Refresh file tree if viewing that project |
| `session-imported` | `{ project_id, session_id }` | Show new session in Overview tab, animate highlight |
| `project-registered` | `{ project_id }` | Add to sidebar list |
| `search-index-updated` | `{ project_id }` | No visible reaction — search results will be fresh on next query |
| `watcher-error` | `{ project_id, error }` | Show subtle error indicator on the project |

**Implementation**:
```rust
// Backend (Rust)
app.emit("project-git-changed", GitChangedPayload { project_id, branch, is_dirty })?;

// Frontend (Svelte)
import { listen } from '@tauri-apps/api/event';
listen('project-git-changed', (event) => {
  updateProjectGitStatus(event.payload);
});
```

**Consequences**:
- Real-time updates without polling
- Native Tauri pattern — well-supported and documented
- Events are fire-and-forget from the backend — no acknowledgment needed
- Frontend can ignore events for projects not currently visible (optimization)
- Event payloads kept small — IDs and changed fields only, not full entity data

---

## Architecture Summary

### All Decisions

| # | Topic | Decision |
|---|-------|----------|
| 001 | Storage | Hybrid: SQLite + filesystem |
| 002 | Search | Tantivy for FTS |
| 003 | Data location | `app_data_dir()` (Tauri platform API) |
| 004 | File access | SQLite for taurhaus-owned data, filesystem for existing content |
| 005 | Platform | Windows first, Linux for development |
| 006 | Hero content | Session/README toggle in Overview tab |
| 007 | Project PK | UUID, auto-generated |
| 008 | Activity state | Computed on read from `last_activity_at` |
| 009 | Session fields | Core typed + extensible metadata JSON blob |
| 010 | Relationships | Fully automatic detection, opt-out |
| 011 | IPC style | Fine-grained commands (~25 total) |
| 012 | Markdown | Frontend rendering with Shiki |
| 013 | Git engine | libgit2 (git2 crate) |
| 014 | Project discovery | Directory scan + auto-registration |
| 015 | Module structure | 8 Rust modules with clear boundaries |
| 016 | Auto-handoff | SessionEnd hook with agent |
| 017 | Manual handoff | /handoff skill as fallback |
| 018 | Handoff format | Markdown + YAML frontmatter + JSON sidecar |
| 019 | CC data | Module designed for v1.1, no UI in v1 |
| 020 | File watcher | notify + ignore crates, pre-filtered watches |
| 021 | Scan watching | Depth-1 watch on scan dirs, auto-register |
| 022 | Event delivery | Tauri events from backend to frontend |

### Open Questions (Deferred)

- Virtual scrolling library selection for large project lists
- Tantivy index configuration (tokenizer, schema, stored fields)
- Exact Claude Code project hash algorithm for the `claude_code` module
- `.taurhausignore` file format (plain glob patterns? .gitignore syntax?)
- SQLite migration strategy (embedded SQL files vs. Rust migration crate)
- Session handoff skill prompt template (exact wording for the agent hook)
