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
| Python | 3.10+ | `sudo apt install python3 python3-venv` (repo scripts only) |

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

Build, daemon, mesh, and release workflows are standardized in `justfile`. Use `just` recipes instead of raw `cargo tauri build`, `bunx tauri build`, or ad hoc cross-compilation commands.

The repo's Python tooling (`just infographics`, `just test-scripts`) needs two
pinned packages. Install them once into a managed environment:

```bash
just python-deps  # scripts/.venv from scripts/requirements.txt (gitignored)
```

`scripts/with-python.sh` picks that environment up for every Python recipe, and
tells you to run `just python-deps` if the dependencies are missing. Set
`TAURHAUS_PYTHON` to use an interpreter that already has them instead.

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

Session isolation is enabled in `e2e/wdio.conf.js`:

- `TAURHAUS_DATA_DIR` is set to a per-session temp app-data directory
- `TAURHAUS_CLAUDE_DIR` is set to a per-session temp Claude root
- A per-session fixture git repo is created for deterministic onboarding and validation flows

Useful E2E env knobs:

- `E2E_PROJECTS_DIR` — project scan root used by E2E helpers
- `E2E_TAURHAUS_PROJECT_PATH` — stable taurhaus fixture path used in duplicate-path tests
- `E2E_INSTALL_DAEMON=1` — opt-in daemon reinstall for E2E recipes

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
