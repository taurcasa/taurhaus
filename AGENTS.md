# taurhaus

Desktop tool for AI project management. Tauri 2 + Svelte 5 + Rust backend + Tailwind v4.

## Stack

- **Backend**: Rust (Tauri 2), SQLite + tantivy full-text search, libgit2 via `git2` crate
- **Frontend**: Svelte 5 (runes only: `$state`, `$derived`, `$effect`, `$props`), Tailwind v4, Geist font
- **IPC**: Fine-grained commands (~25+), one per operation, frontend calls in parallel
- **File watching**: `notify` + `ignore` crates, pre-filtered by .gitignore

## Scope Discipline

Only modify files directly related to your assigned task. Do NOT refactor, clean up, or "improve" code you weren't asked to touch.

## Task Assignment Expectations

Every task assignment should include explicit scope boundaries:

- `In scope`: what to change for this task
- `Out of scope`: what must not be changed as part of this task

If scope is unclear, default to the narrowest interpretation that satisfies the acceptance criteria.

## Pre-Completion Quality Gate

**Before reporting any implementation task as done**, run:

```
just check-quick
```

This runs `cargo check --tests`, frontend typecheck, and frontend unit tests. It must pass before you mark a task complete.

In team/agent workflows, this is the only verification command agents should run.
Do **NOT** run `just check`; team-lead runs the full gate in serialized mode.

Completion criteria in team/agent workflows:

- If `just check-quick` passes on files you touched, report task complete.
- If `just check-quick` fails on files you did **not** touch, report the failure to team-lead and mark your task complete anyway.
- The full quality gate (`just check`) is the team-lead's responsibility.

## Sprint-Start Preflight

At sprint start, confirm these operating rules:

1. Canonical verification lane: `just check-quick`.
2. Full-gate owner: team-lead runs `just check`.
3. Out-of-scope failure rule: report unrelated failures and move on.

## Code Standards

- **Production quality from day one.** Clean foundations steer future code quality.
- **Svelte 5 runes only**: `$state`, `$derived`, `$effect`, `$props`. No legacy stores or reactive syntax.
- **Tailwind v4 with `@theme` tokens**: Custom design tokens defined in `app.css`.
- **Semantic HTML**: `<aside>` for sidebar, `<main>` for content, `<nav>` for navigation.
- **No over-engineering**: Don't abstract until there's actual duplication. Three similar lines beat a premature abstraction.

## Orchestration Protocol

Use these conventions for all team orchestration messages (Claude and Codex agents).

### Assignment Checklist (Mandatory)

- Objective in one sentence.
- Exact deliverable path or output contract.
- Concrete first action (imperative verb + specific file/path).
- Completion signal: "mark task complete and report".
- Explicit response expectation (`no_response_needed` where applicable).

### Message Prefix Convention

- `ACTION REQUIRED:` for messages that require immediate execution.
- `INFO ONLY:` for context that does not need a response; end with "no response needed".

### Anti-Pattern Rules

- No pure acknowledgments to active assignees.
- No "check messages" without explicit action.
- No split assignment across multiple micro-messages.
- No "You have task X assigned" framing; give direct execution instructions.

### Embedded `orchestration_v1` Template

```text
[orchestration_v1]
intent: assign|nudge|info|close
task_id: <task-id-or-empty>
no_response_needed: true|false
first_step: <single concrete first action>
deliverable: <artifact path or output contract>
completion_signal: <required completion response>
precedence: <ordered policy list>
[/orchestration_v1]

<human-readable body>
```

Full protocol design: `docs/architecture/orchestration-protocol-design.md`

## Build & Test

All builds use `just` recipes. Never use raw `cargo tauri build` or cross-compilation.

| Recipe | What it does |
|--------|-------------|
| `just dev` | Full Tauri dev mode (frontend + backend hot-reload) |
| `just check-quick` | Fast iteration gate: `cargo check --tests` + frontend typecheck + frontend unit tests |
| `just check` | Full quality gate (fmt + lint + typecheck + tests). Team-lead only in serialized runs, or pre-release. |
| `just test` | All tests (Rust + frontend unit). Does NOT include E2E. |

Rust backend lives in `src-tauri/`. Cargo commands must run from `src-tauri/` or use `--manifest-path`.

**Vitest**: Must run from the project root (`/home/mstie/projects/taurhaus`), NOT from `src-tauri/`.

## JS Tooling (Bun Required)

- Use `bun install`, `bun run`, and `bunx` for JavaScript/package workflows in this repo.
- Do not use `npm` or `npx` in this repo.

## TDD

- **Test-first for logic** (red -> green -> refactor)
- Rust: `#[test]` + `pretty_assertions` + `tempfile`
- Frontend: Vitest + JSDOM + `@testing-library/svelte`
- Test data generated on the fly in tempdirs, never checked-in fixtures
- AC-driven coverage -- every acceptance criterion gets a test

## Integration Test Shims

The integration tests in `src-tauri/tests/` include source files via `#[path = "../src/..."]` and define stub modules for dependencies that don't exist in the test crate scope. If you add new imports to modules under `coordination/pipelines/` (or other coordination modules included by `coordination/mod.rs`), you must also update the shim modules in:

- `src-tauri/tests/coordination_integration.rs`
- `src-tauri/tests/coordination_onboarding_linux_e2e.rs`

`just check-quick` will catch this via `cargo check --tests`.

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
  mesh_cli.rs     -- Mesh binary/path discovery and command helpers
  orchestrator.rs -- CoordinationOrchestrator public entry points
  runtime.rs      -- Runtime/pane/process integration helpers
  validation.rs   -- Team/member naming and input validation
  backend/        -- BackendSelector, MeshBridgedBackend, FakeBackend
  orchestrator/   -- Lifecycle handlers and internal orchestration modules
  pipelines/      -- Initialize/add/resume pipeline modules
  stores/         -- TeamConfigStore + MemberRuntimeStore (file-locked persistence)
  delivery.rs     -- DeliveryRenderer (onboarding templates, operator notices)
  events.rs       -- Event producer/consumer for inbox/config changes
  health/         -- Health state machine (state/transition/policy)
  reconcile.rs    -- State drift reconciliation
  audit.rs        -- Audit event logging
  consumer.rs     -- Event consumer loop
```

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

## Context Compaction Recovery

If you experience context compaction and either (a) have no new unread messages or (b) are unclear about your current task, **immediately message team-lead and ask for your current assignment**. Do not assume you are done — compaction may have dropped your active task context.

```
mesh send --team taurhaus-team --to team-lead "Context compacted. I have no clear active assignment after compaction. What task should I work on now?"
```

## Mesh Integration

- Teams stored at `~/.claude/teams/`
- Mesh CLI (`mesh`) used for inter-agent communication
- Non-Claude agents get onboarding via mesh daemon notification
- `CliTool` enum: Claude, Codex, Gemini (in `session_scanner/cli_tool.rs`)
