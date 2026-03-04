# taurhaus

Desktop tool for AI project management. Tauri 2 + Svelte 5 + Rust backend + Tailwind v4.

## Stack

- **Backend**: Rust (Tauri 2), SQLite + tantivy full-text search, libgit2 via `git2` crate
- **Frontend**: Svelte 5 (runes only: `$state`, `$derived`, `$effect`, `$props`), Tailwind v4, Geist font
- **IPC**: Fine-grained commands (~25+), one per operation, frontend calls in parallel
- **File watching**: `notify` + `ignore` crates, pre-filtered by .gitignore

## Scope Discipline

Only modify files directly related to your assigned task. Do NOT refactor, clean up, or "improve" code you weren't asked to touch.

## Pre-Completion Quality Gate

**Before reporting any implementation task as done**, run:

```
just agent-quality
```

This runs `cargo fmt`, `cargo clippy`, and `cargo check --tests`. All three must pass before you mark a task complete. If clippy or check fails, fix the issues in your changed files before reporting done.

This is mandatory for every task that touches Rust code. Do not skip it.

## Code Standards

- **Production quality from day one.** Clean foundations steer future code quality.
- **Svelte 5 runes only**: `$state`, `$derived`, `$effect`, `$props`. No legacy stores or reactive syntax.
- **Tailwind v4 with `@theme` tokens**: Custom design tokens defined in `app.css`.
- **Semantic HTML**: `<aside>` for sidebar, `<main>` for content, `<nav>` for navigation.
- **No over-engineering**: Don't abstract until there's actual duplication. Three similar lines beat a premature abstraction.

## Build & Test

All builds use `just` recipes. Never use raw `cargo tauri build` or cross-compilation.

| Recipe | What it does |
|--------|-------------|
| `just dev` | Full Tauri dev mode (frontend + backend hot-reload) |
| `just check` | Full quality gate: clippy + svelte-check + all tests (Rust + frontend unit) |
| `just test` | All tests (Rust + frontend unit). Does NOT include E2E. |

Rust backend lives in `src-tauri/`. Cargo commands must run from `src-tauri/` or use `--manifest-path`.

**Vitest**: Must run from the project root (`/home/mstie/projects/taurhaus`), NOT from `src-tauri/`.

## TDD

- **Test-first for logic** (red -> green -> refactor)
- Rust: `#[test]` + `pretty_assertions` + `tempfile`
- Frontend: Vitest + JSDOM + `@testing-library/svelte`
- Test data generated on the fly in tempdirs, never checked-in fixtures
- AC-driven coverage -- every acceptance criterion gets a test

## Integration Test Shims

The integration tests in `src-tauri/tests/` include source files via `#[path = "../src/..."]` and define stub modules for dependencies that don't exist in the test crate scope. If you add new imports to `coordination/pipelines.rs` or similar included files, you must also update the shim modules in:

- `src-tauri/tests/coordination_integration.rs`
- `src-tauri/tests/coordination_onboarding_linux_e2e.rs`

The `just agent-quality` recipe will catch this via `cargo check --tests`.

## Architecture

- **Storage**: SQLite (metadata) + tantivy (search) + filesystem (source of truth)
- **Git**: libgit2 via `git2` crate, in-process
- **Coordination module**: `src-tauri/src/coordination/` -- team orchestration, mesh integration
- **Session scanner**: `src-tauri/src/session_scanner/` -- multi-tool session detection (Claude, Codex, Gemini)
- **Terminal management**: `src-tauri/src/terminal.rs` -- tmux pane management
- **Feature gating**: `mesh-bridged-backend` Cargo feature (default enabled) gates coordination code

## Key Files

| File | Purpose |
|------|---------|
| `src/Shell.svelte` | Main app layout (titlebar, sidebar, content) |
| `src/app.css` | Design tokens + global styles |
| `src/lib/ipc.js` | Tauri IPC commands + dev-mode mock fallbacks |
| `src-tauri/src/lib.rs` | App setup, IPC command registration |
| `src-tauri/src/commands/coordination.rs` | Coordination IPC commands |
| `src-tauri/src/coordination/` | Coordination subsystem (orchestrator, stores, delivery, etc.) |
| `docs/mesh-view-design.md` | Mesh View design document |
| `docs/phase-4-architecture.md` | Technical architecture (22 ADRs) |

## Coordination Module Structure

```
src-tauri/src/coordination/
  mod.rs          -- module declarations
  domain.rs       -- core types (Member, MemberRole, TeamConfig, HealthState)
  errors.rs       -- CoordinationError enum
  requests.rs     -- IPC DTOs and delivery request types
  state.rs        -- Lazy CoordinationState bootstrap (AppState managed)
  orchestrator.rs -- CoordinationOrchestrator (create/disband/add/remove/deliver)
  stores.rs       -- TeamConfigStore + MemberRuntimeStore (file-locked persistence)
  backend/        -- BackendSelector, MeshBridgedBackend, FakeBackend
  delivery.rs     -- DeliveryRenderer (onboarding templates, operator notices)
  events.rs       -- Event producer/consumer for inbox/config changes
  health.rs       -- Health state machine
  reconcile.rs    -- State drift reconciliation
  audit.rs        -- Audit event logging
  consumer.rs     -- Event consumer loop
```

## Logging

- **Backend**: `tracing::info/warn/error/debug` -- goes to stderr + log file
- **Frontend**: `console.log` -- monkey-patched to also write to backend log via IPC

## Mesh Integration

- Teams stored at `~/.claude/teams/`
- Mesh CLI (`mesh`) used for inter-agent communication
- Non-Claude agents get onboarding via mesh daemon notification
- `CliTool` enum: Claude, Codex, Gemini (in `session_scanner/cli_tool.rs`)
