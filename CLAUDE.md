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
- **No over-engineering**: Don't abstract until there's actual duplication. Three similar lines beat a premature abstraction.

## Logging

Unified logging pipeline — both frontend and backend write to a single `taurhaus.log` file in `app_data_dir()`. Truncated on each app launch.

| Layer | How to log | Where it goes |
|-------|-----------|---------------|
| **Frontend** | `console.log/warn/error/debug` | WebView console + backend log file via IPC |
| **Backend** | `tracing::info/warn/error/debug` | stderr + log file |
| **Daemon** | `tracing::info/warn/error/debug` | stderr (daemon's own process) |

**Frontend→backend bridge**: `src/lib/logger.js` (imported first in `main.js`) monkey-patches `console.*` to also call the `frontend_log` IPC command. This means `console.log` in frontend code already writes to the backend log file — no special import needed. Always use `console.log` for frontend logging, never a custom function.

**Log format**: `[HH:MM:SS.mmm] [INF|WRN|ERR|DBG] [frontend] message` for frontend lines, standard `tracing_subscriber` format for backend lines.

**Key files**: `src/lib/logger.js` (bridge), `src-tauri/src/commands/logging.rs` (IPC handler).

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

## Layout Dimensions

| Element | Size | Notes |
|---------|------|-------|
| Titlebar | 46px tall | Logo + tab pill + controls |
| Sidebar | 252px wide | Matches logo area in titlebar |
| Panel gap | 6px | `gap-1.5` between sidebar and main |
| Frame padding | 6px | `p-1.5` around panels inside frame |
| Tab pill | 36px tall | `rounded-t-lg`, connects to main panel |

## Build & Development

All builds use `just` recipes. Never use raw `cargo tauri build`, `npx tauri build`, or cross-compilation toolchains.

**Development (runs in WSL/Linux):**

| Recipe | What it does |
|--------|-------------|
| `just dev` | Full Tauri dev mode (frontend + backend hot-reload) |
| `just dev-frontend` | Frontend dev server only (no Rust backend) |
| `just check` | Full quality gate: clippy + svelte-check + all tests (Rust + frontend unit) |
| `just test` | All tests (Rust + frontend unit). Does NOT include E2E. |
| `just build-daemon` | Builds the WSL daemon binary (Linux target, runs in WSL2) |
| `just install-daemon` | Builds + copies daemon to `~/.local/bin/` |
| `just bump VERSION` | Bump version in all files (tauri.conf.json, Cargo.toml, package.json, Cargo.lock, CHANGELOG.md) |
| `just release` | Create GitHub Release from current version. Pushes to remote, uploads artifacts. |

**E2E tests (run on target platform, NOT in WSL):**

E2E tests launch the real app binary via tauri-driver + WebDriverIO. They run on Linux only — Windows E2E is not supported (shared app data directory + tantivy index corruption makes reliable isolation impractical).

| Recipe | Where it runs | What it does |
|--------|--------------|-------------|
| `just test-e2e` | Linux (WSL) | Tier 1 E2E — builds Linux debug binary, runs specs locally. |
| `just test-e2e-full` | Linux (WSL) | Tier 1 + Tier 2 (requires daemon running) |
| `just test-e2e-spec SPEC` | Linux (WSL) | Single spec file (e.g. `just test-e2e-spec search-workflow`) |
| `just test-macos-e2e` | **macOS** via SSH | macOS E2E test suite on remote Mac Mini. |

**Platform builds (run natively on target OS):**

| Recipe | What it does |
|--------|-------------|
| `just build-windows` | Syncs to `D:\taurhaus_build`, builds NSIS installer natively on Windows via `cmd.exe`. |
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
4. just check                   # run full quality gate
5. just build-windows           # build Windows NSIS installer
6. just build-macos             # build macOS DMG
7. just release                 # push, create GitHub release, upload artifacts
```

The `release` recipe enforces: must be on `main`, working tree must be clean, tag must not already exist. Never replace assets on an existing release — if a fix is needed, bump the version and release again.

**Important**: The Windows exe is built **natively on Windows** via WSL2 interop (`cmd.exe`). We do NOT cross-compile from Linux. Never use `--target x86_64-pc-windows-msvc` from WSL, `cargo xwin`, or any cross-compilation approach. The `just build-windows` recipe handles everything — sync, npm install, and native Windows cargo build.

**macOS**: The macOS app is built **natively on a Mac Mini** (Scaleway, arm64) via SSH. We do NOT cross-compile from Linux. The `just build-macos` recipe handles everything — rsync sync, npm install, daemon build + codesign, and `cargo tauri build`. The Mac's PATH requires a login shell (`zsh -ilc`) for fnm/cargo/homebrew.

If the build fails with "Access is denied" on the exe, the app is still running — close it first, then rebuild.

**Vitest cwd gotcha**: Vitest must run from the project root (`/home/mstie/projects/taurhaus`), NOT from `src-tauri/`. If `npx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this, but if running vitest manually, always `cd` to the project root first.

## Architecture Summary

- **Storage**: SQLite (metadata, sessions, relationships) + tantivy (full-text search) + filesystem (source of truth for content)
- **Data location**: Tauri `app_data_dir()` — platform-appropriate
- **IPC**: Fine-grained commands (~25). One per operation. Frontend calls in parallel.
- **Git**: libgit2 via `git2` crate. In-process, no CLI dependency.
- **Markdown**: Frontend rendering with Shiki (VS Code grammars). Raw text over IPC.
- **File rendering**: Classification → IPC → cache → render. See [`docs/file-rendering-pipeline.md`](docs/file-rendering-pipeline.md).
- **File watching**: `notify` + `ignore` crates. Pre-filtered by .gitignore. Git internals debounced 2s.
- **Session handoffs**: Auto-created via Claude Code `SessionEnd` hook (agent type). Markdown + YAML frontmatter + JSON sidecar. `/handoff` skill as manual fallback.
- **Relationships**: Auto-detected from project signals (Cargo.toml deps, CLAUDE.md refs, session mentions). Opt-out, not opt-in.
- **Platform**: Windows first (release builds), Linux/WSL2 for development.

Full architecture: [`docs/phase-4-architecture.md`](docs/phase-4-architecture.md) (22 ADRs)

## Key Files

| File | Purpose |
|------|---------|
| `src/Shell.svelte` | Main app layout (titlebar, sidebar, content) |
| `src/App.svelte` | Entry wrapper |
| `src/app.css` | Design tokens + global styles |
| `src/lib/ipc.js` | Tauri IPC commands + dev-mode mock fallbacks |
| `docs/design-brief.md` | Full requirements (Phase 2) |
| `docs/phase-4-architecture.md` | Technical architecture (22 ADRs) |
| `docs/images/system-architecture.jpg` | System architecture infographic |
| `docs/file-rendering-pipeline.md` | File viewing/rendering pipeline + asset cache |
| `docs/images/file-rendering-pipeline.jpg` | File rendering pipeline infographic |
| `BOOTSTRAP.md` | Project lifecycle and phase status |

## Development Workflow (Phase 5)

Full workflow: [`docs/phase-5-workflow.md`](docs/phase-5-workflow.md) | Infographic: [`docs/workflow-infographic.jpg`](docs/workflow-infographic.jpg)

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
- `just check` runs full gate: clippy + svelte-check + all tests
- Full test suite on every task. E2E at milestones.
- Visual review (frontend tasks): 8 categories, scored 1-10, **min 9 per category**
- Visual dual review: self-review + Gemini Pro 3 cross-review. Lower score wins, Claude is final arbiter with justified override.

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

Phases 1-4 complete. Phase 5 (Implementation) is next. See `BOOTSTRAP.md` for details.
