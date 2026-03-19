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
just check-quick  # Fast gate: cargo check --tests + typecheck + frontend unit tests
```

Core release-lane recipes (from `justfile`):

| Recipe | Purpose |
|--------|---------|
| `just test` | Full non-E2E test lane (Rust + frontend) |
| `just test-fast` | Fast iteration lane (`cargo check --tests` + frontend tests) |
| `just check-quick` | Per-task fast gate (`cargo check --tests`, `typecheck`, frontend tests) |
| `just check` | Full quality gate (`fmt`, `lint`, `typecheck`, `test`) for team-lead serialized runs or pre-release/PR validation |
| `just metrics` | Quality KPI report (tests, coverage, build health, code size, E2E inventory) |
| `just test-visual` | Browser-mode visual screenshot lane for mocked component states |
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
    shell/              # Shell-level controllers (shortcuts, events, window, tmux)
    ipc/                # IPC domain modules (client, projects, sessions, templates, etc.)
  Shell.svelte          # Main app layout
  App.svelte            # Entry point with splash screen
  app.css               # Design tokens and global styles
src-tauri/              # Rust backend
  src/
    commands/           # Tauri IPC command handlers
    coordination/       # Multi-agent team orchestration (mesh CLI, pipelines)
    daemon/             # WSL daemon (launcher, protocol, server)
    session_scanner/    # Multi-CLI session detection
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
