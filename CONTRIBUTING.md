# Contributing to taurhaus

Thanks for your interest in contributing! This guide covers everything you need to get started.

## Development Environment

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| WSL2 | Ubuntu recommended | `wsl --install` |
| Rust | Latest stable | [rustup.rs](https://rustup.rs/) |
| Bun | Latest stable | [bun.sh](https://bun.sh/) |
| just | Latest | `cargo install just` |
| tmux | 3.0+ | `sudo apt install tmux` |
| Python | 3.10+ with PyYAML and Pillow | `sudo apt install python3 python3-yaml python3-pil` (`just infographics` only) |

### Setup

```bash
git clone https://github.com/taurcasa/taurhaus.git
cd taurhaus
bun install
just dev  # Full Tauri dev mode with hot-reload
```

For frontend-only development (no Rust backend):

```bash
just dev-frontend
```

### Parallel lane worktrees

Use `just provision-worktree PATH BRANCH [BASE]` from the main checkout whenever
you create a development lane. `BASE` defaults to `origin/main`; the recipe
fetches first, creates the branch worktree, runs `bun install --frozen-lockfile`
inside it, and gives only that worktree a Cargo config pointing at the shared
`~/.cache/taurhaus-lane-target`. Cargo's own locking safely serializes concurrent
lane builds. The main checkout and release builds continue using their existing
`src-tauri/target` directories.

After the branch is merged, run `just remove-worktree PATH`. It removes the
worktree and branch, but refuses an unmerged branch unless you explicitly set
`FORCE_BRANCH=1`. Run `just clean-lane-target` whenever you need to reclaim the
shared Cargo cache; deleting it is always safe.

Build, daemon, mesh, and release workflows are standardized in `justfile`. Use `just` recipes instead of raw `cargo tauri build`, `bunx tauri build`, or ad hoc cross-compilation commands.

Nothing in the app needs an API key. One developer tool does: regenerating the
documentation infographics. Copy `.env.example` to `.env` in the repo root and
fill in `OPENAI_API_KEY` when you need it — `.env` is gitignored, and the key is
read only by `scripts/generate-infographics.py` (`just infographics`), never by
the app or the daemon. See [Infographic regeneration](docs/operations/infographics.md).

## Code Standards

### Svelte 5

- **Runes only**: `$state`, `$derived`, `$effect`, `$props`. No legacy stores or reactive syntax.
- **Dark mode via `$derived` tokens**: All color switching through named derived variables. Never inline ternaries for colors in templates.
- **Semantic HTML**: `<aside>` for sidebar, `<main>` for content, `<nav>` for navigation.

### Tailwind v4

- Custom design tokens defined in `app.css` using `@theme`.
- Document any non-standard arbitrary values.
- If you define a color shade (e.g., `success-500`), define every shade you reference in class names.

### Rust

- Standard Rust conventions: `clippy` clean, `rustfmt` formatted.
- Error handling via `thiserror` for typed errors.
- Tests use `#[test]` with `pretty_assertions` and `tempfile` for temp dirs.
- Command-layer modules keep tests in sibling `tests.rs` files; lower-level modules keep inline `#[cfg(test)] mod tests`.

### General

- No over-engineering: don't abstract until there's actual duplication.
- No backwards-compatibility hacks for removed code.
- Keep solutions simple and focused on what was asked.

## Testing

Use fast verification during implementation:

```bash
just check-quick  # Fast gate: cargo fmt + cargo check --tests + typecheck + frontend unit tests
```

Core release-lane recipes (from `justfile`):

| Recipe | Purpose |
|--------|---------|
| `just test` | Full non-E2E test lane (Rust + frontend) |
| `just test-fast` | Fast iteration lane (`cargo check --tests` + frontend tests) |
| `just check-quick` | Per-task fast gate (`cargo fmt`, `cargo check --tests`, `typecheck`, frontend tests) |
| `just check` | Full quality gate (`fmt`, `lint`, `typecheck`, `test`) for team-lead serialized runs or pre-release/PR validation |
| `just metrics` | Quality KPI report (tests, coverage, build health, code size, E2E inventory) |
| `just test-visual` | Browser-mode visual screenshot lane for mocked component states |
| `just visual-shot C S [V] [T] [OUT]` | One visual-host fixture shot at a real window size through Edge headless, for viewport-anchored surfaces the 960x640 browser lane cannot judge; `just visual-shot-stop` stops only the server it started |
| `just test-macos` | Rust tests on the remote Mac Mini |
| `just test-macos-e2e` | macOS E2E suite on the remote Mac Mini |
| `just agent-quality` | Agent-facing wrapper around `just check-quick` |

In team/agent workflows, run this before declaring completion:

```bash
just check-quick
```

Use `just check` only when a full serialized gate is explicitly required, typically by the team lead or during release preparation.

Individual test suites:

```bash
just test-rust       # Rust tests (fast compile check + unit + integration)
just test-frontend   # Frontend tests (Vitest)
```

Frontend tests run from the project root (not `src-tauri/`). Vitest is configured to find test files at the root level.

If you add imports to source files included by integration shims (for example modules under `coordination/pipelines/`), update shim modules in `src-tauri/tests/` and rerun `just check-quick` to catch test-crate scope breakage early.

### E2E Testing

Run E2E from the project root:

```bash
just test-e2e
just test-e2e-full
just test-e2e-spec search-workflow
```

E2E recipes are safe-by-default: they no longer force `install-daemon` (which can kill/restart a live daemon). To opt in explicitly:

```bash
E2E_INSTALL_DAEMON=1 just test-e2e
```

Each WDIO worker is fully isolated by `e2e/wdio.conf.js` and `e2e/helpers/workerEnv.js`.
Every writable product root is pointed inside that worker's session temp directory:

- `HOME` — an isolated login home, so account discovery cannot reach the operator's dot-directories
- `TAURHAUS_DATA_DIR` — per-session app-data directory
- `TAURHAUS_CLAUDE_DIR` — per-session Claude root
- `CODEX_HOME`, `GROK_HOME`, and the taurhaus-only `TAURHAUS_AGY_DIR` — per-session tool homes
- `TAURHAUS_DAEMON_PORT` — a private daemon port derived from the session root and probed for
  availability in 20000-31999; port 17233 belongs to the operator and is never used
- `TMUX_TMPDIR` — the worker's own tmux *server* socket directory, with any inherited `TMUX`
  removed (a client inside a tmux pane resolves the socket from `$TMUX` and ignores
  `TMUX_TMPDIR`); teardown kills that server (`e2e/helpers/laneTmux.js`)
- A per-session fixture git repo is created for deterministic onboarding and validation flows

Process cleanup is ownership-checked (`e2e/helpers/laneCleanup.js`): a run token is inherited by
the driver, WebKitWebDriver, app and daemons, and every live process is recorded as PID plus
Linux `/proc` start time in a checkout-scoped ledger. Cleanup kills only identities whose PID
*and* start time still match, so a concurrent run and a reused PID are both left alone. One disclosed exception: clearing a worker's own driver ports falls back to a port-derived process pattern (`4500 + pid % 300`), so two concurrent runs that collide on a derived port are not isolated from each other on that path.

`e2e/specList.js` is the sealed spec manifest: every non-paid `e2e/specs/*.js` file must belong
to a named group, and `e2e/specList.test.js` fails on an ungrouped file. The three paid Codex
lanes (`compaction-codex-hooks`, `managed-stage-codex`, `managed-stage-deadline`) are never in
a default suite run and must be named on the command line.

Useful E2E env knobs:

- `E2E_PROJECTS_DIR` — project scan root used by E2E helpers
- `E2E_TAURHAUS_PROJECT_PATH` — stable taurhaus fixture path used in duplicate-path tests
- `E2E_INSTALL_DAEMON=1` — opt-in daemon reinstall for E2E recipes
- `E2E_CODEX_SOURCE_HOME` — the Codex home a paid lane copies `auth.json` from

### Regression Testing

Every regression fix **must** include a test that would have caught the regression:

1. Write the test first — confirm it fails against the broken code (red)
2. Fix the regression (green)
3. The test stays permanently as a guard

Where to put regression tests:

- **E2E regressions** (visual, behavioral): `e2e/specs/regressions.js`
- **Rust regressions**: `#[test]` in the affected module with `// Regression:` comment
- **Frontend unit regressions**: in the relevant `.test.js` with `// Regression:` comment

Each regression test must document what broke and why (commit reference if available).

## Pull Request Process

1. Create a feature branch from `main`
2. Make your changes with clear, focused commits
3. Run `just check-quick` during implementation; run `just check` only when a full serialized gate is required
4. Open a PR with a clear title and description
5. Include a brief test plan (what you tested and how)

### Updating the bundled mesh release

When a Taurhaus change depends on a mesh source change, keep the embedded build metadata and compatibility lock aligned:

1. Bump mesh's patch version and refresh its `Cargo.lock`.
2. Run `just check` in the mesh repository.
3. Commit mesh, then run `just build-release` so `mesh version --json` reports the committed source revision.
4. From Taurhaus, run `just update-mesh-lock <version> <protocol_version> <schema_version> <git_commit>` with the exact JSON values.
5. Run `just bundle-mesh` and `just mesh-verify-lock`.
6. Run `just install-mesh`, then restart running member daemons so the development host uses the lock-matching binary.
7. Commit `src-tauri/resources/mesh.lock.json`, `mesh.manifest.json`, and `mesh.version` with the Taurhaus change. Use the normal Taurhaus release recipes afterward.

If the mesh repository has no configured remote, stop after the local commit; do not invent a push target.

### Commit Messages

- Template: `<type>(<scope>): <summary> (#task-id)`
- Use imperative mood: "Add feature" not "Added feature"
- First line: concise summary (under 72 characters)
- Optional body: explain "why" not "what"

### What Makes a Good PR

- Focused on a single concern
- Tests for new behavior
- No unrelated changes mixed in
- Both light and dark mode considered for UI changes

## Project Structure

```
src/                    # Svelte frontend
  lib/                  # Shared components and utilities
    components/         # Reusable UI components (mesh, templates, shell panels)
    shell/              # Shell-level controllers (daemon status, events, navigation, project
                        #   selection, session lifecycle, shortcuts, state bridge, theme, window)
    ipc/                # IPC domain modules (client, projects, sessions, templates, etc.)
  Shell.svelte          # Main app layout
  App.svelte            # Entry point with splash screen
  app.css               # Design tokens and global styles
src-tauri/              # Rust backend
  src/
    commands/           # Tauri IPC command handlers
    coordination/       # Multi-agent team orchestration (mesh CLI, pipelines)
    daemon/             # Companion daemon: WSL on Windows, native on macOS/Linux
                        #   (launcher, protocol, server, session-activity hub, compaction,
                        #   codex-notify, agy-hooks, usage-poller, auth, watch)
    session_scanner/    # Multi-CLI session detection (registry in cli_tool.rs; accounts/
                        #   and idle/ hold the per-tool provider and activity slices)
    services/           # Shared backend services (task queries, task sync)
    db/                 # SQLite queries and migrations
    git/                # libgit2 wrapper
    search/             # tantivy full-text search
docs/                   # Design docs, architecture, guides
e2e/                    # WebdriverIO end-to-end tests
```

## Getting Help

- Check existing [issues](../../issues) before filing a new one
- For architecture questions, see [ARCHITECTURE.md](ARCHITECTURE.md)
- For technical deep-dives, see docs in the [docs/](docs/) directory

## Code of Conduct

Be respectful, constructive, and welcoming. We follow the [Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).
