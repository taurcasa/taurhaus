# Phase 3C: User Journey Mapping

> Formalized from the [Design Brief](design-brief.md) workflows using the [Domain Understanding](phase-3b-domain.md) entity inventory and action vocabulary.

---

## Step 1: User Roles

### R-01: Developer (Primary — v1 only)

**Relationship to the system**: The Developer uses taurhaus to maintain awareness of all their AI-assisted projects — where things stand, what happened last session, where to find information — so they can start productive Claude Code sessions without manual archaeology.

**Primary role**: Yes. All journeys optimize for this role.

**Concurrent tools**: Claude Code (terminal), web browser, file manager. taurhaus is a companion running alongside these, typically in a side panel on an ultrawide monitor.

---

## Step 2: Journey Candidates

Derived from walking the entity inventory (E-01 through E-04) and action vocabulary:

1. Orient across projects (E-01: browse, filter, sort)
2. Resume project context (E-01: view detail, E-02: view current session, implicit: git commits)
3. Reference docs mid-session (E-03: browse tree, view/render)
4. Search across projects (E-01, E-02, E-03: search)
5. End session / create handoff (E-02: import, add notes)
6. Register new project (E-01: register)
7. Manage project relationships (E-04: create, edit, remove)
8. Configure taurhaus settings (Settings: edit ignore patterns, display options)
9. First-run setup (E-01: batch register via scan)

---

## Step 3: Priority Scoring

| # | Journey | Frequency | Impact | Score | Priority |
|---|---------|-----------|--------|-------|----------|
| 1 | Orient across projects | 5 (daily, multiple) | 5 (blocking — can't start work without it) | 25 | Primary |
| 2 | Resume project context | 5 (every session start) | 5 (blocking — session quality depends on it) | 25 | Primary |
| 3 | Reference docs mid-session | 5 (many times per session) | 3 (important but workaround exists: terminal) | 15 | Primary |
| 4 | Search across projects | 4 (several times daily) | 3 (important but can browse instead) | 12 | Secondary |
| 5 | End session / create handoff | 4 (end of every session) | 5 (blocking — lost context without it) | 20 | Primary |
| 6 | Register new project | 2 (monthly) | 3 (important but infrequent) | 6 | Secondary |
| 7 | Manage project relationships | 1 (ad-hoc, rare) | 1 (nice to have) | 1 | Tertiary |
| 8 | Configure settings | 1 (rare) | 1 (nice to have) | 1 | Tertiary |
| 9 | First-run setup | 1 (once ever) | 5 (blocking — can't use the app without it) | 5 | Secondary |

**Primary journeys** (score 15+): J-01, J-02, J-03, J-05
**Secondary journeys** (score 6-14): J-04, J-06, J-09
**Tertiary journeys** (score <6): J-07, J-08

---

## Step 4: Detailed Journey Maps

### J-01: Orient Across Projects

**Role**: Developer
**Priority**: Primary
**Frequency**: Multiple times daily (score 5). Variable entry — full scan, direct navigation, or chronological review.
**Volume**: 30-50 projects visible simultaneously, max 100+.

**Trigger**: Start of a work session, returning from a break, or switching mental context. The user opens taurhaus (or glances at it in the side panel) to get a sense of the landscape.

**Goal**: "I need to know where things stand across all my projects so I can decide where to focus."

**Steps**:
1. **Glance at project dashboard** — Needs: all projects visible with activity indicators (last commit date, clean/dirty status, current branch). Decision: none yet, just scanning. Result: mental model of project landscape forming.
2. **Identify active vs. stale projects** — Needs: visual differentiation of activity states (active/recent/stale/dormant). Time-since-last-activity must be scannable without reading dates. Decision: none, filtering. Result: attention narrows to recently active projects.
3. **Check for projects needing attention** — Needs: dirty working tree indicators, unusual branch states, any user-set flags/notes. Decision: "Does anything need urgent attention?" Information needed: working tree status, branch name (unexpected branches signal WIP), user notes. Result: urgent items identified (or confirmed none).
4. **Select a project** — Needs: enough context per project to make the choice (name, activity, status, maybe relationship indicators). Decision: "Which project should I work on?" Most common outcome: pick the most recently active project or the one matching today's plan. Result: project selected.
5. **Drill into project detail** — Needs: transition to project detail view. Context carried: project identity. Result: user is now in J-02 (Resume Project Context) or browsing the project detail.

**Key decisions**:
- "Where should I focus?" — Needs: recency of activity (primary), project relationships (secondary), user notes/flags (secondary). Options: pick the hottest project, resume where they left off, or address something flagged. Most common: resume the project from the most recent session.

**Resolution**: A project is selected and the user is looking at its detail view. They know what to work on.

**Error paths**:
- **No projects registered**: First-run state. Redirect to J-09 (First-Run Setup).
- **All projects stale**: Not an error — legitimate state. Dashboard should still feel informative, not broken.
- **File watcher not running / stale data**: Show "last updated" timestamp. Offer manual refresh.

**Notes**: This journey has multiple entry modes. Sometimes the user does a full visual scan (steps 1-4). Sometimes they already know which project to work on and go directly to step 5 (skip the scan). Sometimes they want a chronological view across projects ("what happened recently across everything?") rather than a per-project scan. The view must support all three modes.

---

### J-02: Resume Project Context

**Role**: Developer
**Priority**: Primary
**Frequency**: Every session start, 1-5 times daily (score 5).
**Volume**: 1 project in focus. 1 current session, 5-20 historical. ~50 recent commits. Dozens of docs.

**Trigger**: About to start a Claude Code session on a specific project. The user has selected a project (possibly via J-01) and needs to rebuild mental context.

**Goal**: "I need to understand where this project left off so I can start a productive coding session."

**Steps**:
1. **Read the most recent session handoff** — Needs: session summary, next steps, open questions prominently displayed. This is the highest-value content on this view. Decision: none initially, just absorbing context. Result: user knows what happened last time and what was planned next.
2. **Check open questions and decisions** — Needs: open questions and decisions from the session, clearly distinguished from the narrative summary. Decision: "Are any of these still relevant?" Result: user confirms or mentally updates the plan.
3. **Scan recent git history** — Needs: commit log with messages, dates, and maybe file-change summaries. Last ~10-20 commits are most relevant. Decision: "Has anything changed since the last session?" (commits by other tools, manual edits, etc.) Result: user sees if the codebase matches what the session described.
4. **Review project docs if needed** — Needs: quick access to key docs (CLAUDE.md, README, design docs). Not always needed — sometimes the handoff is sufficient. Decision: "Do I need more context than the handoff provides?" Result: user reads relevant docs or skips this step.
5. **Mentally commit to a plan** — Needs: all the above synthesized. Decision: "Is the last session's plan still valid, or do I need to adjust?" Most common: plan is valid, proceed. Result: user has enough context to start coding.

**Key decisions**:
- "Is the plan still valid?" — Needs: session next steps + recent commits + project state. If commits exist that the session didn't anticipate, the plan may need adjustment.
- "What doc should I check?" — Needs: file tree or recent files list. Usually CLAUDE.md or the most recently modified doc.

**Resolution**: The user has enough context to start a productive Claude Code session. They switch to the terminal and begin.

**Error paths**:
- **No sessions exist for this project**: Empty session area. Show "No sessions yet" with explanation of how sessions are created (via Claude Code handoff skill). Fall back to git history and docs as the only available context.
- **Session file malformed / parse error**: Show raw content with a warning. Don't block the view.
- **Git history unavailable**: Show whatever is available. Degrade gracefully — sessions and docs may still be useful without git data.

**Notes**: This journey often follows J-01 directly. The transition from dashboard → project detail should feel seamless, with the most recent session immediately visible (no extra clicks to find it).

---

### J-03: Reference Docs Mid-Session

**Role**: Developer
**Priority**: Primary
**Frequency**: Highest frequency — many times per session, throughout the day (score 5).
**Volume**: 1 project's docs (20-thousands of files). Usually looking for a specific file.

**Trigger**: Working in Claude Code and needs to check a document, source file, or image. The user's hands are on the keyboard, attention is split. Speed matters.

**Goal**: "I need to find and read a specific document without losing my flow."

**Steps**:
1. **Switch to taurhaus** — Needs: taurhaus already open in side panel or alt-tabbable. No startup delay. Decision: none. Result: taurhaus is in focus.
2. **Navigate or search for the file** — Needs: either (a) file tree already showing the right project, or (b) search bar accessible immediately. Decision: "Do I know where the file is?" If yes → navigate tree. If no → search. Most common: user knows the approximate location and navigates, or uses search for cross-project lookup. Result: file located.
3. **Read/view the content** — Needs: file rendered appropriately (markdown rendered, source syntax-highlighted, images displayed). Must be readable in a side panel (1280px wide). Decision: none — pure consumption. Result: user has the information.
4. **Return to Claude Code** — Needs: nothing from taurhaus. Alt-tab back. Result: back to coding with the needed information.

**Key decisions**:
- "Navigate or search?" — Needs: visible file tree for navigation, visible search bar for searching. Both must be accessible without mode-switching.

**Resolution**: The user found and read the information they needed. They're back in Claude Code.

**Error paths**:
- **File not found**: If navigating, file may have been deleted. Show file tree state with indication. If searching, no results — suggest broadening search.
- **File can't be rendered**: Show raw content. Binary files show type/size info rather than garbage.
- **Wrong project in view**: User needs to switch projects. Must be fast — project switcher always accessible.

**Notes**: This is the most time-sensitive journey. The entire round trip (switch to taurhaus → find doc → read → switch back) should take seconds, not minutes. The UI must not require the user to "set up" the view before they can find what they need. Whatever project they were last looking at should still be visible. Context preservation is critical.

---

### J-04: Search Across Projects

**Role**: Developer
**Priority**: Secondary (score 12, but v1 critical feature)
**Frequency**: Several times daily (score 4).
**Volume**: Searching across all projects — potentially thousands of files, hundreds of sessions, thousands of commits.

**Trigger**: The user needs to find something but doesn't remember which project it's in. "Where did we define that pattern?" "Which project has the auth config?"

**Goal**: "I need to find a specific piece of information somewhere across all my projects."

**Steps**:
1. **Open search** — Needs: search accessible from any view. Keyboard shortcut (Cmd/Ctrl+K or similar). Decision: none. Result: search input focused and ready.
2. **Type query** — Needs: responsive search-as-you-type with debounce. Results should start appearing immediately. Decision: none. Result: results streaming in.
3. **Scan results** — Needs: results grouped or labeled by type (doc, commit, session, code) and by project. Each result shows: project name, file path or entity type, matching snippet with highlighted terms. Decision: "Which result is the one I need?" Needs enough context per result to judge relevance without clicking into each one. Result: target result identified.
4. **Click into result** — Needs: clicking a result navigates to the appropriate view (doc viewer, commit detail, session detail) with the search match visible/highlighted. Decision: none. Result: user is viewing the full content.
5. **Return to results or continue** — Needs: back button or search results preserved for trying another result. Decision: "Was this the right result?" If no, go back to step 3. Result: either found what they needed or trying another result.

**Key decisions**:
- "Which result is correct?" — Needs: project name (which project), entity type (what kind of thing), file path (where exactly), snippet (preview of matching content). All visible per result without expanding.

**Resolution**: The user found the information. They're either viewing it in taurhaus or have navigated to the relevant project/doc.

**Error paths**:
- **No results**: "No results for [query]". Suggest alternative terms or broader search.
- **Too many results**: Results must be ranked by relevance. Filtering by project or type helps narrow down.
- **Search index stale**: Show results from available index with "Index may be outdated" indicator. Offer rebuild.

**Notes**: Search is intentionally designed as a first-class feature, not a bolt-on. It searches across docs, source code, commit messages, and session content. Results are unified but distinguishable by type. Performance target: results in <200ms for search-as-you-type.

---

### J-05: End Session (Create Handoff)

**Role**: Developer
**Priority**: Primary (score 20)
**Frequency**: End of every session, 1-5 times daily (score 4).
**Volume**: 1 session being created. 1 project.

**Trigger**: Finishing a Claude Code session. The user wants to capture context for next time.

**Goal**: "I need to record what happened this session so future-me can pick up seamlessly."

**Steps**:
1. **Run handoff skill in Claude Code** — Needs: nothing from taurhaus. User runs a Claude Code slash command. Result: structured handoff file written to the project's directory.
2. **taurhaus auto-detects the handoff file** — Needs: file watcher running, detecting new handoff files. Decision: none (system-initiated). Result: session appears in taurhaus.
3. **Verify handoff imported** — Needs: taurhaus shows the new session in the project detail. Visual indicator that a new session arrived (notification, highlight, auto-scroll). Decision: none — just confirming. Result: user sees the session is captured.
4. **Optionally enrich the handoff** — Needs: ability to add personal notes, diagrams, or images to the session. Inline editing in the session detail. Decision: "Is there anything worth adding beyond what the skill captured?" Most common: no — the auto-generated handoff is sufficient. Result: session is complete.

**Key decisions**:
- "Should I add notes?" — Needs: visible "add notes" affordance in the session view. Not required — most sessions are created and left as-is. When used, typically for visual diagrams, screenshots, or personal observations the skill couldn't capture.

**Resolution**: The handoff is visible in taurhaus. The user can close the session confidently, knowing context is preserved.

**Error paths**:
- **Handoff file not detected**: File watcher may have missed it. Manual refresh or "import handoff" action as fallback.
- **Handoff file malformed**: Show warning but import what's parseable. Don't fail silently.
- **taurhaus not running**: Handoff file still exists on disk. When taurhaus next starts, it should detect and import pending handoffs.

**Notes**: The creation happens outside taurhaus (in Claude Code). taurhaus's role is detection, import, and optional enrichment. The user's interaction with taurhaus in this journey is brief — mostly verification and occasionally adding notes.

---

### J-06: Register New Project

**Role**: Developer
**Priority**: Secondary (score 6)
**Frequency**: Monthly or when discovering new projects (score 2).
**Volume**: 1 project at a time (manual) or 10-50 at a time (batch scan).

**Trigger**: User has a new project in `~/projects/` that should be tracked, or is setting up taurhaus for the first time.

**Goal**: "I need to add a project to taurhaus so I can track it."

**Steps**:
1. **Initiate registration** — Needs: "Add project" action accessible from dashboard. Decision: manual path entry or browse/scan? Result: registration flow started.
2. **Specify project path** — Needs: path input with filesystem browser or autocomplete. Validate that path exists and is a git repo. Decision: none. Result: path entered and validated.
3. **Review / edit metadata** — Needs: auto-populated fields (name from directory, description from README if available, type inferred from contents). Editable. Decision: "Is the auto-detected metadata correct?" Most common: yes, accept defaults. Result: metadata confirmed.
4. **Confirm registration** — Needs: summary of what will be registered. Decision: none (confirmation). Result: project registered, indexed, and appears on dashboard.

**Key decisions**:
- "Manual or scan?" — For single project: manual path. For first-run or catching up: scan `~/projects/` for all git repos.

**Resolution**: Project appears on the dashboard with initial index complete.

**Error paths**:
- **Path doesn't exist**: Validation error on input.
- **Not a git repo**: Warning — taurhaus requires git. Suggest initializing git.
- **Already registered**: Inform user, don't duplicate.
- **Scan finds many repos**: Show list with checkboxes. User selects which to register.

---

### J-09: First-Run Setup

**Role**: Developer
**Priority**: Secondary (score 5 — once ever, but blocking)
**Frequency**: Once (score 1). Impact: 5 (blocking).
**Volume**: 15-50 projects to scan and register.

**Trigger**: User opens taurhaus for the first time. No projects registered.

**Goal**: "I need to set up taurhaus with all my existing projects."

**Steps**:
1. **See empty state with clear guidance** — Needs: not a blank screen. Welcome message explaining what taurhaus does and how to start. Primary action: "Scan ~/projects/ for git repositories." Decision: none. Result: user knows what to do.
2. **Run project scan** — Needs: point to a directory (default: ~/projects/). Scan for git repos. Show progress. Decision: "Which directory to scan?" Default is usually correct. Result: list of discovered projects.
3. **Review discovered projects** — Needs: list of found repos with auto-detected names and descriptions. Checkboxes to include/exclude. Decision: "Which of these should be tracked?" Most common: all of them. Result: selection confirmed.
4. **Initial indexing** — Needs: progress indicator as projects are indexed (git history, file trees, content). May take 5-30 seconds depending on project count and size. Decision: none (waiting). Result: indexing complete.
5. **Arrive at populated dashboard** — Needs: smooth transition from setup to the main dashboard, now populated with projects. Result: user is oriented and can start using taurhaus normally.

**Key decisions**:
- "Which projects to track?" — Needs: list with names and paths. User can deselect repos they don't want tracked (e.g., archived experiments, forks).

**Resolution**: Dashboard is populated. taurhaus is ready for daily use.

**Error paths**:
- **No git repos found**: "No git repositories found in ~/projects/. You can add projects manually." Link to J-06.
- **Scan permissions error**: Some directories unreadable. Skip with warning, continue scanning.
- **Indexing fails for some projects**: Register what succeeded. Show errors for failures. Don't block the whole setup.

---

## Tertiary Journeys (Brief Descriptions)

### J-07: Manage Project Relationships

**Role**: Developer. **Frequency**: Ad-hoc, rare. **Priority**: Tertiary.
Create, edit, or remove directional links between projects. Accessed from project detail. Simple form: select target project, select type, optional description. Low volume (2-5 per project). No complex workflow — just CRUD on a lightweight entity.

### J-08: Configure Settings

**Role**: Developer. **Frequency**: Rare. **Priority**: Tertiary.
Adjust global taurhaus settings: ignore patterns, display preferences, scan directories. Accessed via settings nav item. Sectioned form. Set once, rarely revisited.

---

## Step 5: Cross-Journey Patterns

### Shared Entities

| Entity | Journeys | Role |
|--------|----------|------|
| E-01: Project | J-01, J-02, J-03, J-04, J-05, J-06, J-09 | Central to everything. Appears in every journey. |
| E-02: Session | J-02, J-04, J-05 | Primary in J-02 (resume context) and J-05 (create handoff). Searchable in J-04. |
| E-03: Document | J-02, J-03, J-04 | Primary in J-03 (reference docs). Supporting in J-02 (review project docs). Searchable in J-04. |
| E-04: Relationship | J-01, J-07 | Supporting in J-01 (relationship indicators on dashboard). Primary only in J-07 (manage). |
| Git Commit (implicit) | J-01, J-02, J-04 | Supporting in J-01 (activity signal). Important in J-02 (recent changes). Searchable in J-04. |

### Shared Steps

| Step pattern | Journeys | Component implication |
|-------------|----------|---------------------|
| "Browse/scan a list of projects" | J-01, J-06, J-09 | Shared project list component |
| "View project detail" | J-01→J-02, J-03, J-04 (result click) | Shared project detail layout |
| "Search for something" | J-03, J-04 | Shared search component (global search bar) |
| "Navigate file tree" | J-02, J-03 | Shared file tree component |
| "Read rendered document" | J-02, J-03, J-04 (result click) | Shared document viewer component |
| "View session content" | J-02, J-05 | Shared session viewer component |

### Journey Sequences

Common sequences (one journey flows into another):

1. **J-01 → J-02**: Orient → Resume. Most common daily sequence. Dashboard → pick project → deep-dive into context.
2. **J-02 → J-03**: Resume → Reference. Reading handoff, need more detail from a doc.
3. **J-03 → J-04**: Reference → Search. Looking for a doc, realize it might be in a different project.
4. **J-05 → J-01**: End session → Orient. Finished one project, decide what's next.
5. **J-09 → J-01**: First-run → Orient. Setup complete, start exploring.

### Conflicting Needs

| Conflict | Journeys | Resolution direction |
|----------|----------|---------------------|
| Dashboard: scannable overview vs. detailed per-project info | J-01 (needs overview) vs. J-02 (needs detail) | Master-detail pattern: list provides overview, detail panel provides depth. J-01 stays in the list; J-02 focuses on the detail panel. |
| Document access: tree navigation vs. search | J-03 (knows where file is → tree) vs. J-04 (doesn't know → search) | Both available simultaneously. Tree for known locations, search for discovery. Not mutually exclusive. |
| Side panel (1280px) vs. center (2560px) use | J-03 (glance at side panel) vs. J-02 (focused reading in center) | Responsive layout. Core info fits in 1280px. Expanded reading benefits from 2560px. Layout adapts, not degrades. |

---

## Step 6: Validation

### Coverage Check

| Entity | Journey coverage |
|--------|-----------------|
| E-01: Project | J-01, J-02, J-03, J-04, J-05, J-06, J-07, J-09 — covered |
| E-02: Session | J-02, J-04, J-05 — covered |
| E-03: Document | J-02, J-03, J-04 — covered |
| E-04: Relationship | J-01 (indicators), J-07 (manage) — covered |
| Git Commit (implicit) | J-01 (activity), J-02 (history), J-04 (search) — covered |
| Settings | J-08 — covered |

All entities appear in at least one journey. No orphaned entities.

### Completeness Check

Every primary and secondary journey has: trigger, goal, detailed steps with information needs, decisions, resolution, error paths, frequency, and volume. Pass.

### Priority Validation

Top 4 journeys by score (J-01: 25, J-02: 25, J-05: 20, J-03: 15) align with the design brief's stated user priorities: orienting (J-01), resuming context (J-02), creating handoffs (J-05), and referencing docs (J-03). Pass.

### Boundary Check

Each journey is self-contained with a clear start and end. Journey sequences are documented but each journey can stand alone. Pass.

---

## Handoff to Phase 3D

This document provides the inputs for Information Architecture:

- **9 journey documents** (4 primary, 3 secondary, 2 tertiary) → view inventory derivation
- **Cross-journey patterns** (shared entities, steps, sequences) → shared component identification
- **Conflicting needs** → design tensions to resolve in view design
- **Journey sequences** → navigation path mapping
- **Frequency and volume data** → navigation tier assignments
