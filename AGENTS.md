# taurhaus

Desktop tool for AI project management. Single clear view into all projects — code, docs, progress, history — so you never lose context between sessions.

## Stack

Tauri 2 + Svelte 5 + Rust backend + Tailwind v4. Same stack as MIR. Geist font family.

## Design Paradigms

- **Snappy**: Every interaction feels instant. No loading spinners, no layout shifts. Optimistic UI everywhere.
- **Dense but calm**: Compact layout for ultrawide side-panel use alongside Claude Code. Breathing room where it matters — don't pack for packing's sake.
- **Floating Panel layout**: Dark teal frame (`bg-brand-950`) wraps the entire window. Sidebar and main content are distinct rounded panels floating inside it.
- **One dark teal**: Frame and sidebar share `bg-brand-950`. No shade variations between them — one color, one identity.
- **Manila Folder tabs**: The active tab pill uses the same background as the main content panel, creating visual continuity between tab and content.
- **Inverse scoop**: Where the tab pill meets the dark frame on the right, a concave corner (CSS inverse border-radius) creates a smooth transition.
- **Theme toggle stays visible**: Light/Dark switch lives in the titlebar, always accessible. Not hidden in settings.
- **Custom titlebar**: No OS window decorations. The titlebar is part of our UI. All non-interactive titlebar space is draggable (`data-tauri-drag-region`).

## Code Standards

- **Production quality from day one.** Clean foundations steer future code quality.
- **Svelte 5 runes only**: `$state`, `$derived`, `$effect`, `$props`. No legacy stores, no legacy reactive syntax.
- **Dark mode via `$derived` tokens**: All color switching through named `$derived` variables. Never inline ternaries for colors in the template.
- **Tailwind v4 with `@theme` tokens**: Custom design tokens defined in `app.css`. Document any non-standard arbitrary values.
- **Semantic HTML**: `<aside>` for sidebar, `<main>` for content, `<nav>` for navigation, `<section>` for content sections.
- **Rust test placement**: Command-layer modules use an external sibling `tests.rs`; lower-level modules keep inline `#[cfg(test)] mod tests`.
- **Bun-only JS workflows**: Use `bun install`, `bun run`, and `bunx`. Do not use `npm` or `npx` in this repo.
- **No over-engineering**: Don't abstract until there's actual duplication. Three similar lines beat a premature abstraction.
- **Shared repo workflow**: This repo is routinely dirty because multiple agents and the team lead work in parallel. Ignore unrelated modified or untracked files in `git status`; only reason about files you are touching.
- **When to escalate repo changes**: Never stop just because unrelated files changed. Escalate only if the same files you need to edit changed unexpectedly during your task, or if external changes block verification.

## Team Messaging Conventions

Use these conventions for day-to-day team messaging. This section is operational guidance, not a protocol schema.

### Assignment Contract

Use the five-line assignment contract in
[`docs/team-delivery-standard.md`](docs/team-delivery-standard.md). That standard
owns the objective, deliverable, first action, completion signal, and review
route; this file adds no competing assignment fields.

### Message Prefix Convention

Use the [message conventions in the team delivery
standard](docs/team-delivery-standard.md#message-conventions). This repository
adds no alternate prefixes or response-expectation rules.

### Anti-Pattern Rules

- No pure acknowledgments to active assignees.
- No "check messages" without explicit action.
- No split assignment across multiple micro-messages.
- No "You have task X assigned" framing; give direct execution instructions.

## Mesh Team Coordination (for mesh-bridged agents: Codex, Antigravity, Grok)

When you are part of a Mesh team, use the **explicit lifecycle commands** — not `mesh task update` — for all task work. `task update` is a legacy compatibility command that loses workflow meaning.

### Task lifecycle commands

```bash
mesh task accept ID --team TEAM --name NAME           # Seen and understood
mesh task start ID --active-form "Working..." --team TEAM --name NAME  # Work begun
mesh task progress ID --summary "Update" --team TEAM --name NAME       # Interim update
mesh task block ID --reason "Why" --team TEAM --name NAME              # Blocked
mesh task review ID --summary "Ready" --team TEAM --name NAME          # Handoff
mesh task complete ID --summary "Result" --team TEAM --name NAME       # Done
```

### Action-first reply behavior

When you receive an assignment message, **start working immediately**. Do NOT:
- Send an acknowledgment before doing the work
- Summarize the task back to the sender
- Reply with "understood" or "working on it"

The assignment message IS your work order. Execute it, then report completion.

### Reading messages

```bash
mesh read --unread --mark-read --team TEAM --name NAME
```

`mesh read --unread` only shows real inbox messages. If it returns no messages, check `mesh tasks` or `mesh task get` for assigned work.

### Replying and sending

```bash
mesh send RECIPIENT "message" --team TEAM --name NAME --summary "brief"
```

### Environment variables (set once, skip repetitive flags)

```bash
export MESH_TEAM="my-team"
export MESH_NAME="my-agent"
```

### Lead vs agent command surface

| Role | Commands |
|------|----------|
| **Lead** | `task create`, `task assign`, `nudge`, `xteam send/relay`, `--as-lead` repair mutations |
| **Agent** | `accept`, `start`, `progress`, `block`, `review`, `complete` |
| **Both** | `send`, `read`, `tasks`, `task get`, `who`, `heartbeat`, `status` |

Leads can also use `--override-lane-limit` on `task assign` / `task start` and `--admin-reason` on `task assign` / `block` / `review` / `complete` for audit-trailed repair mutations.

## Logging

Unified structured logging pipeline:

- Canonical sink: `app_data_dir()/taurhaus.log.jsonl` (or `<TAURHAUS_DATA_DIR>/taurhaus.log.jsonl` when overridden).
- Format: newline-delimited JSON records (JSONL), one event per line.
- Backend sink model: single async writer pipeline in `src-tauri/src/commands/logging.rs` (bounded channel + writer thread), append-only.
- Rotation/retention: size-based rotation at 20 MB segments, 7-day retention for rotated files.

| Layer | How to log | Where it goes |
|-------|-----------|---------------|
| **Frontend** | `console.log/warn/error/debug` | WebView console + structured IPC payload to JSONL sink |
| **Backend** | `tracing::info/warn/error/debug` plus `emit_global(...)` structured events | stderr + JSONL sink |
| **Daemon bridge points** | backend daemon RPC lifecycle emits `daemon.rpc.*` events | JSONL sink (`daemon_request_id`, `method`, `status`, timing fields) |

**Frontend→backend bridge**: `src/lib/logger.js` (imported first in `main.js`) monkey-patches `console.*` and sends structured payloads to `frontend_log` IPC. Payloads include `component`, `subsystem`, `event`, `message`, and optional context/correlation fields. Use `console.*`; do not create bypass loggers.

**Correlation fields**:

- `run_id`: generated once per app run, attached to every JSONL record.
- `request_id`: per IPC command lifecycle (`ipc.command.*`).
- `interaction_id`: frontend interaction chain correlation (logger bridge).
- `daemon_request_id`: backend->daemon RPC correlation (`daemon.rpc.*`).

**Key files**:

- `src/lib/logger.js` (frontend bridge + interaction correlation + drop telemetry)
- `src-tauri/src/commands/logging.rs` (JSONL sink, global emitter, rotation, `frontend_log`)
- `src-tauri/src/commands/lifecycle.rs` (`ipc.command.received/completed/failed`, `ipc.lock.wait`)
- `src-tauri/src/startup/mod.rs` (`startup.phase.started/completed/failed`)
- `src-tauri/src/daemon_api.rs` + `src-tauri/src/provider/daemon_client.rs` (`daemon.rpc.sent/response/timeout`)

**Logging policy**: see [`docs/architecture/log-level-guidelines.md`](docs/architecture/log-level-guidelines.md).

## Svelte 5 Patterns

**Consume-after-capture for signal props**: When an `$effect` reads a prop and then calls a callback that nullifies it in the parent, capture the value into a `const` first. Svelte 5 signals propagate eagerly — the prop becomes null mid-effect otherwise.

```javascript
// WRONG — changedPaths becomes null after consume
$effect(() => {
  if (!changedPaths) return
  onConsumed?.()                         // sets parent signal to null
  doWork(changedPaths)                   // reads null!
})

// RIGHT — capture before consuming
$effect(() => {
  const paths = changedPaths             // capture
  if (!paths) return
  onConsumed?.()                         // safe to consume
  doWork(paths)                          // uses captured value
})
```

**Mesh route geometry lives in `meshLayout.js`**: `MeshCanvas.svelte` should consume explicit route geometry from `src/lib/components/meshLayout.js`, and `MeshConnection.svelte` should render that geometry directly. Do not add new caller-side `bend` math.

## Layout Dimensions

| Element | Size | Notes |
|---------|------|-------|
| Titlebar | 46px tall | Logo + tab pill + controls |
| Sidebar | 252px wide | Matches logo area in titlebar |
| Panel gap | 6px | `gap-1.5` between sidebar and main |
| Frame padding | 6px | `p-1.5` around panels inside frame |
| Tab pill | 36px tall | `rounded-t-lg`, connects to main panel |

## Build & Development

All builds use `just` recipes. Never use raw `cargo tauri build`, `bunx tauri build`, or cross-compilation toolchains.

**Development (runs in WSL/Linux):**

| Recipe | What it does |
|--------|-------------|
| `just dev` | Full Tauri dev mode (frontend + backend hot-reload) |
| `just dev-frontend` | Frontend dev server only (no Rust backend) |
| `just test-visual` | Browser-mode visual screenshot lane for mocked component states. |
| `just metrics` | Quality KPI snapshot (tests, coverage, build health, code size, E2E inventory). |
| `just test` | All non-E2E tests: Rust compile check + Rust unit + Rust integration/system + frontend unit. |
| `just test-fast` | Fast iteration lane: Rust compile check (`cargo check --tests`) + frontend unit tests. |
| `just check-quick` | Fast feedback for iteration: Rust format auto-fix (`cargo fmt`) + Rust compilation (`cargo check --tests`) + frontend typecheck + frontend unit tests. |
| `just check` | Full quality gate: fmt + lint + typecheck + `just test` (all non-E2E tests). Team-lead serialized runs or pre-release only. |
| `just build-daemon` | Builds the WSL daemon binary (Linux target, runs in WSL2) |
| `just install-daemon` | Builds + copies daemon to `~/.local/bin/` |
| `just bump VERSION` | Bump version in all files (tauri.conf.json, Cargo.toml, package.json, Cargo.lock, CHANGELOG.md) |
| `just release` | Create GitHub Release from current version. Pushes to remote, uploads artifacts. |

**E2E tests (run on target platform, NOT in WSL):**

E2E tests launch the real app binary via tauri-driver + WebDriverIO. They run on Linux only — Windows E2E is not supported (shared app data directory + tantivy index corruption makes reliable isolation impractical).

| Recipe | Where it runs | What it does |
|--------|--------------|-------------|
| `just test-e2e` | Linux (WSL) | Tier 1 E2E — safe-by-default (does not auto-run `install-daemon`), builds Linux debug binary, runs specs locally. |
| `just test-e2e-full` | Linux (WSL) | Tier 1 + Tier 2 (requires daemon running), safe-by-default daemon handling |
| `just test-e2e-spec SPEC` | Linux (WSL) | Single spec file (e.g. `just test-e2e-spec search-workflow`), safe-by-default daemon handling |
| `just test-macos-e2e` | **macOS** via SSH | macOS E2E test suite on remote Mac Mini. |

For local runs that should rebuild/reinstall the daemon first, opt in explicitly: `E2E_INSTALL_DAEMON=1 just test-e2e`.
E2E sessions also use isolated roots via `TAURHAUS_DATA_DIR` and `TAURHAUS_CLAUDE_DIR`, plus fixture path knobs `E2E_PROJECTS_DIR` and `E2E_TAURHAUS_PROJECT_PATH`.

Agent/team workflow rule:
- Use `just check-quick` during implementation.
- Do **NOT** run `just check` as an agent; team-lead owns serialized full-gate runs.

**Platform builds (run natively on target OS):**

| Recipe | What it does |
|--------|-------------|
| `just build-windows` | Syncs to `C:\taurhaus_build` by default (override with `TAURHAUS_WINDOWS_BUILD_DIR`), then builds the NSIS installer natively on Windows via a PowerShell wrapper. |
| `just build-macos` | Syncs via rsync to Mac Mini, builds `.app` + `.dmg` natively via SSH. |
| `just build-macos-universal` | Universal macOS binary (arm64 + x86_64) on remote Mac. |
| `just sync-macos` | Sync source to remote Mac Mini. |
| `just test-macos` | Run Rust tests on remote Mac Mini via SSH. |

### Release Workflow

Always use the `just` recipes for releases. Never manually create GitHub releases or upload assets.

```
1. just bump 0.4.0              # bump all version files + add CHANGELOG section
2. Edit CHANGELOG.md            # fill in changes for this version
3. git add -A && git commit     # commit the version bump
4. just check                   # full gate before release (team-lead serialized run)
5. just build-windows           # build Windows NSIS installer
6. just build-macos             # build macOS DMG
7. just release                 # push, create GitHub release, upload artifacts
```

The `release` recipe enforces: must be on `main`, working tree must be clean, tag must not already exist. Never replace assets on an existing release — if a fix is needed, bump the version and release again.

**Important**: The Windows exe is built **natively on Windows** via WSL2 interop (`powershell.exe -File` into the synced Windows workspace). We do NOT cross-compile from Linux. Never use `--target x86_64-pc-windows-msvc` from WSL, `cargo xwin`, or any cross-compilation approach. The `just build-windows` recipe handles everything — sync, Bun install, and the native Windows Tauri build.

**macOS**: The macOS app is built **natively on a Mac Mini** (Scaleway, arm64) via SSH. We do NOT cross-compile from Linux. The `just build-macos` recipe handles everything — rsync sync, Bun install, daemon build + codesign, and `cargo tauri build`. The Mac's PATH requires a login shell (`zsh -ilc`) for bun/cargo/homebrew.

If the build fails with "Access is denied" on the exe, the app is still running — close it first, then rebuild.

**Vitest cwd gotcha**: Vitest must run from the project root, NOT from `src-tauri/`. If `bunx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this, but if running vitest manually, always `cd` to the checkout root first.

**Manual visual review host**: `bun run dev:visual` starts the Vite visual fixture host for mocked component states. Use it for rapid layout iteration; use `just test-visual` for automated screenshot coverage.

## Architecture Summary

- **Storage**: SQLite (metadata, sessions, relationships) + tantivy (full-text search) + filesystem (source of truth for content)
- **Data location**: Tauri `app_data_dir()` by default; `TAURHAUS_DATA_DIR` can override for test/dev isolation
- **Harness model**: Four registered CLI harnesses — `claude` (Claude Code), `codex` (Codex CLI), `agy` (Antigravity CLI), `grok` (Grok CLI). Per-tool code lives in capability slices behind `src-tauri/src/session_scanner/cli_tool.rs`; never branch on tool identity outside those slices. See `docs/architecture/harness-model.md`.
- **IPC**: Fine-grained commands (currently 90 in `src-tauri/src/lib.rs` generate_handler). One per operation; frontend fans out in parallel.
- **Daemon protocol**: `PROTOCOL_VERSION = 19` in `src-tauri/src/daemon/protocol.rs`; app and daemon must match exactly. 11 made the account methods generic, 12 replaced the retired Google tool value with `agy`, 13 added `grok`, 14 retired the Codex compaction mode, 15 moved the deadline pass into the daemon, 16 moved team initialization into the daemon, 17 moved add/resume/stop into the daemon, 18 moved resume-team/reonboard into the daemon, and 19 moved standalone team create/disband and roster edits into the daemon.
- **Accounts and usage**: Per-tool providers in `src-tauri/src/session_scanner/accounts/`. Every registered harness has an account provider (`cli_tool.rs`, `account_provider()`) — these are independent capabilities, not one gate. `account_selector`/`account_selection` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GROK_HOME`) is what enables *switching*: Antigravity declares neither, so its provider describes one implicit account and no chooser, chip or submenu appears. The separate `usage` flag decides whether a *usage* provider exists (Claude, Codex, Antigravity; Grok is `usage: false` and shows the registry's note where a meter would be). Choices live in `project_tool_accounts` (migration 013). Tokens are never logged, persisted, or refreshed by taurhaus.
- **Git**: libgit2 via `git2` crate. In-process, no CLI dependency.
- **Markdown**: Frontend rendering with Shiki (VS Code grammars). Raw text over IPC.
- **File rendering**: Classification → IPC → cache → render. See [`docs/file-rendering-pipeline.md`](docs/file-rendering-pipeline.md).
- **File watching**: `notify` + `ignore` crates. Pre-filtered by .gitignore. Git internals debounced 2s.
- **Session handoffs**: Auto-created via Claude Code `SessionEnd` hook (agent type). Markdown + YAML frontmatter + JSON sidecar. `/handoff` skill as manual fallback.
- **Relationships**: Auto-detected from project signals (Cargo.toml deps, CLAUDE.md refs, session mentions). Opt-out, not opt-in.
- **Team templates**: Git-backed role/preset storage + `MeshTeamBuilder`-driven setup flow (quick presets, role filters, drag-and-drop roster editing) with advanced catalog/history in `TemplateBrowserPanel`, while preserving the existing initialize payload contract.
- **Windows Mesh behavior**: Background `wsl`/mesh/tmux launches intentionally suppress console windows, and Mesh runtime/project matching relies on normalized Windows, WSL UNC, and Linux path forms rather than raw string equality.
- **Platform**: Windows first (release builds), Linux/WSL2 for development.

Full architecture: [`ARCHITECTURE.md`](ARCHITECTURE.md) and [`docs/architecture/`](docs/architecture/) references.

## Key Files

| File | Purpose |
|------|---------|
| `src/Shell.svelte` | Main app layout (titlebar, sidebar, content) |
| `src/App.svelte` | Entry wrapper |
| `src/app.css` | Design tokens + global styles |
| `src/lib/ipc.js` | Thin compatibility re-export. Real IPC implementations live in `src/lib/ipc/`. |
| `src/lib/ipc/` | Frontend IPC domain modules (`client`, `projects`, `sessions`, `tasks`, `templates`, `coordination`, `system`) plus payload/mocks modules. |
| `src/lib/context/` | Frontend context providers (`ProjectContext.js`, `SessionContext.js`). |
| `src/lib/HoverCard.svelte` | Sidebar hover preview focused on current activity, latest change, and relationship cues. |
| `src/lib/components/MeshTab.svelte` | Mesh orchestration state machine (gate/setup/init/runtime) |
| `src/lib/components/meshTabController.svelte.js` | Controller state/actions for `MeshTab.svelte`. |
| `src/lib/components/MeshCanvas.svelte` | Runtime node canvas that consumes `meshLayout.js` output. |
| `src/lib/components/meshLayout.js` | Pure mesh canvas layout engine for node boxes and explicit connection routes. |
| `src/lib/components/MeshConnection.svelte` | SVG cubic-route renderer fed by explicit control points from `meshLayout.js`. |
| `src/lib/components/MeshSetupView.svelte` | Gate/empty/setup/initializing shell that hosts the primary team-builder surface. |
| `src/lib/components/MeshTeamBuilder.svelte` | Primary team setup UI with quick presets, role filters, drag-and-drop roster composition, and inline validation. |
| `src/lib/components/TemplateBrowserPanel.svelte` | Advanced role/preset catalog, import/export, history, and diff entry points. |
| `src/lib/components/templateBrowserController.svelte.js` | Controller state/actions for template browsing/composition. |
| `src/lib/components/TeamCustomizerPanel.svelte` | Advanced preset/draft editor used from the template catalog flow. |
| `src/lib/components/TemplateHistoryPanel.svelte` | Template commit history, diff, dirty status, and revert UI |
| `src/lib/components/templateHistoryController.svelte.js` | Controller state/actions for template history/diff/revert. |
| `src-tauri/src/startup/` | App bootstrap pipeline (`bootstrap`, `daemon`, `search`, `watchers`). |
| `src-tauri/src/services/task_query.rs` | Shared task query service for backend consumers. |
| `src-tauri/src/services/task_sync.rs` | Task synchronization service for daemon/IPC flows. |
| `src-tauri/src/daemon_api.rs` | Daemon process API wrapper used by commands/startup flows. |
| `src-tauri/src/project_provider.rs` | Active project resolution/provider utilities. |
| `src-tauri/src/provider/platform_paths.rs` | Central authority for app data, team roots, daemon binary, log path, and Claude hook paths. |
| `src-tauri/src/coordination/pipelines/` | Coordination domain pipelines (`initialize`, `members`, `lifecycle`, `helpers`). |
| `src-tauri/src/coordination/claude_hooks.rs` | Claude `SessionStart(source=compact)` bridge, runtime-aware hook installation, and standalone hook logging. |
| `src-tauri/src/coordination/compact_hook.rs` | Native compaction-hook bridge and managed Codex/Grok hook installers. |
| `src-tauri/src/coordination/stores/compaction.rs` | Compaction delivery idempotency state and audit bookkeeping. |
| `src-tauri/src/session_scanner/transcript_boundary.rs` | Bounded transcript-tail parsing used to timestamp Codex native-hook delivery. |
| `src-tauri/src/templates/adapters.rs` | Role import/export adapters, mapping rules, provenance, and round-trip loss tracking. |
| `src-tauri/src/templates/storage/` | Template git/storage domain split (`roles`, `presets`, `git`, `state`). |
| `docs/coordination-architecture.md` | Coordination subsystem decisions, milestones, and status |
| `ARCHITECTURE.md` | System architecture overview and module map |
| `docs/team-templates.md` | User guide for template authoring/composition/history workflows |
| `docs/testing-guide.md` | Visual testing lane boundaries, usage, and screenshot conventions. |
| `docs/images/system-architecture.jpg` | System architecture infographic |
| `docs/file-rendering-pipeline.md` | File viewing/rendering pipeline + asset cache |
| `docs/images/file-rendering-pipeline.jpg` | File rendering pipeline infographic |
| `CHANGELOG.md` | Shipped milestones and release history |
| `docs/archive/design-workflow.md` | Archived v0.5.x design-first loop; its broad collaboration pattern remains relevant |

## First File To Read By Task

| Task type | Start here |
|-----------|------------|
| Add/modify IPC command | `src-tauri/src/commands/`, then `src-tauri/src/lib.rs` (handler registration), then `src/lib/ipc/` |
| Add/fix a Svelte component | `src/lib/components/` (component file plus matching test in same directory) |
| Fix file watcher behavior | `src-tauri/src/startup/watchers.rs`, `src-tauri/src/fs/watcher.rs`, `src-tauri/src/event_processor.rs` |
| Fix session detection | `src-tauri/src/session_scanner/mod.rs`, `src-tauri/src/session_scanner/idle/`, `src-tauri/src/session_scanner/process.rs` |
| Fix compaction detection / reinjection | `src-tauri/src/coordination/compact_hook.rs`, `src-tauri/src/coordination/stores/compaction.rs`, `src-tauri/src/session_scanner/transcript_boundary.rs`, `src-tauri/src/commands/terminal_settings.rs` |
| Fix path/root resolution | `src-tauri/src/provider/path.rs`, `src-tauri/src/provider/platform_paths.rs` |
| Add database query logic | `src-tauri/src/db/`, then `src-tauri/src/models/mod.rs` |

## Development Workflow (Phase 5)

Workflow reference: [`CONTRIBUTING.md`](CONTRIBUTING.md) (setup and contribution flow) and the sections below.

### Autonomous Execution Loop
- **Project loop**: Work through ALL phases (5A→5B→...→5G) autonomously. No pause between phases.
- **Per phase**: Create ALL tasks upfront → Execute entire backlog → Milestone review → Next phase.
- **Stop conditions**: project complete, user returns, blocked after 7 attempts, major architecture question.
- **Engine**: Ralph Loop manages session continuity across context boundaries.

### TDD
- **Test-first for logic** (red → green → refactor), **visual review for layout**
- Rust: `#[test]` + `pretty_assertions` + `tempfile`. Frontend: Vitest + JSDOM + `@testing-library/svelte`. E2E: WebdriverIO + `tauri-driver`
- AC-driven coverage — every acceptance criterion gets a test, no numeric targets
- Test data generated on the fly in tempdirs, never checked-in fixtures

### Regression Testing
When a regression is discovered — frontend or backend — the fix follows TDD against the regression:
1. **Write a failing test first** that reproduces the exact regression (red)
2. **Fix the bug** (green)
3. **The test stays forever** as a guard against recurrence

- **E2E regressions** go in `e2e/specs/regressions.js` — one `describe` block per regression, comment documents the original commit and root cause
- **Rust regressions** go as `#[test]` in the affected module with a `// Regression:` comment
- **Frontend unit regressions** go in the relevant `.test.js` file with a `// Regression:` comment
- Every regression test must document: what broke, which commit broke it, and why

This is non-negotiable. No regression fix ships without a corresponding test.

### Quality Gates
- `just check-quick` is the per-task gate: `cargo fmt` + `cargo check --tests` + frontend typecheck + frontend unit tests
- `just check` is the full gate and is run by team-lead in serialized fashion (or before release)
- E2E at milestones.
- Visual review (frontend tasks): 8 categories, scored 1-10, **min 9 per category**
- Visual dual review: self-review + a cross-review from the other model family (Opus ↔ Codex, as every PR review loop runs). Lower score wins, the orchestrator is final arbiter with justified override.
- **Design-led UI work** follows the design-first loop: brief → design proposal → approval → implement → review. See the archived v0.5.x process note at [`docs/archive/design-workflow.md`](docs/archive/design-workflow.md). Use `claude-design-lead` for creative direction and `frontend-design-skill-developer` for UI implementation — give the direction lane functional requirements and creative freedom, not pixel-level specs.

### Tasks
- Claude Code native task format (subject, description, status, blocks/blockedBy, metadata)
- All tasks for a phase created upfront before execution begins
- Half-day units. Categories: backend, frontend, integration, e2e, infrastructure
- Iteration: fix immediately, max 7 attempts before flagging user

### AI Autonomy
- **Autonomous**: implementation approach, Rust patterns, minor spec deviations, crate selection, small emergent features, minor arch adjustments within ADR spirit
- **Ask user**: skipping planned features, major ADR contradictions, significant module boundary changes, quality gate failure after 7 attempts
- Spec deviations documented in deviation log, reviewed at milestones

### Security
- `/security-audit` on integration tasks + at every phase boundary (5A–5G)

## Phase Status

Phases 1-4 complete. Implementation continues under task-based milestones; see [`CHANGELOG.md`](CHANGELOG.md) for shipped progress.
