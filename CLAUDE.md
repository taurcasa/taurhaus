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
- `src-tauri/src/startup/telemetry.rs` (the `emit_startup_event` helper plus `startup.app.started`, `startup.paths.resolved`, `startup.logging.initialized`, `startup.database.started/completed/failed`, `startup.daemon_phase.started/completed`, `startup.daemon_connect.succeeded/deferred`, `startup.orchestration.started/completed`, `startup.watchers.initialized`, `startup.search.initialized`, `startup.background_tasks.started/completed`). The rest of the family is emitted where the phase runs: `startup.watchers.failed`/`startup.search.failed` in `startup/orchestration.rs` and `startup/harness.rs`, `startup.watchers.bootstrap.started/completed` in `startup/watchers.rs`, `startup.bootstrap_thread.spawned`, `startup.daemon_bootstrap.started/completed/failed`, `startup.mesh_install.started/completed/failed` and `startup.daemon_protocol.checked` in `startup/daemon.rs`. There is no generic `startup.phase.*` family — a test asserts those legacy names are never emitted.
- `src-tauri/src/daemon_api.rs` + `src-tauri/src/provider/daemon_client.rs` (`daemon.rpc.sent/response/timeout`)
- `src-tauri/src/daemon_lifecycle.rs` (`daemon.repair.started/completed/failed` — one bundled-daemon repair per app/daemon protocol-mismatch episode, with `installed_version`, `bundled_version`, `installed_protocol`, `app_protocol`)
- `src-tauri/src/session_scanner/classification.rs` (`activity.state.changed` with `pid`, `tool`, `from`, `to`, `source`)
- `src-tauri/src/session_scanner/process.rs` (`session_scanner.process_scan.degraded/recovered` — one `degraded` on entry, a bounded 60s reminder while the outage lasts, one `recovered` on exit)
- `src-tauri/src/session_scanner/launch.rs` (`launch.model.*`, `launch.effort.*`, `launch.flag.deprecated`, `launch.selector.ignored`, `launch.selector.rewritten` — the last one info, emitted where the launch base pinned another account dir)
- `src-tauri/src/commands/command_center/launching.rs` + `src-tauri/src/coordination/pipelines/helpers.rs` (`launch.command.rendered`, `launch.account.*`, `launch.base.opaque`, `launch.base.unresolved`)
- `src-tauri/src/coordination/compaction_events.rs` (native-hook delivery bookkeeping: `compaction.injected/skipped/failed`)
- `src-tauri/src/coordination/compact_hook.rs` (hook execution: `compaction.<tool>_hook.received/resolved/delivered/skipped/failed` for `claude`/`codex`/`grok`, where `<tool>` is inferred from grok's reserved `GROK_*` hook env and otherwise from the transcript path; plus `compaction.hook.compat_import` and `compaction.compact_hook.failed`)
- `src-tauri/src/daemon/usage_poller.rs` (`usage.fetched` debug, `usage.failed` warn — never tokens, never a URL with a query string)
- `src-tauri/src/session_scanner/accounts/mod.rs` (`account.provider.floor`) and `accounts/legacy_statusline.rs` (`claude.usage.legacy_bridge.removed`)
- `src-tauri/src/coordination/task_effort.rs` (`effort.resume.started/completed/failed` — the Codex relaunch that puts an assignment's effort into force; budget exhaustion is one `failed` record with `reason: budget_exhausted`, `attempts: 3`, and `task_id`)
- `src-tauri/src/daemon/background_scheduler.rs` (`self_heal.pass.completed/failed` plus the bounded once-per-run `effort.sweep.awaiting_settings` and `effort.sweep.skipped_busy` records — the second says at least one root's effort sweep was skipped because another operation owned that root's orchestrator)
- `src-tauri/src/coordination/task_deadline_pass.rs` + `src-tauri/src/daemon/deadline_scheduler.rs` (`deadline.nudge.sent` / `deadline.task.staled` — one info record per committed deadline action, with `team`, `member`, `task_id`, and `deadline_minutes`; `deadline.pass.completed` / `deadline.pass.failed` — daemon pass liveness and failure records with duration and pass summary/error fields)
- `src-tauri/src/coordination/stores/runtime.rs` (`coordination.runtime.record_skipped` for each unreadable runtime record, with `member` and `reason`; `coordination.runtime.commit_skipped` when a probe-time snapshot changed before commit, with `member` and `changed_fields`)
- `src-tauri/src/coordination/pipelines/helpers.rs` (`coordination.pane.probe_failed` when a transient identity probe preserves the previous pane identity)
- `src-tauri/src/coordination/agy_hooks_installer.rs` (`agy.hooks.degraded`)
- `src-tauri/src/session_scanner/idle/agy.rs` (`agy.identity.resolved` with `source: index|hooks`, emitted once per change)
- `src-tauri/src/commands/terminal_settings.rs` (`compaction.codex_hook.unsupported/version_unknown/reconciled`); `compaction.codex_hook.degraded` also comes from `coordination/compact_hook.rs` and `commands/coordination.rs`
- `src-tauri/src/bin/taurhaus-daemon.rs` (`codex.notify.appended`)
- `src-tauri/src/startup/daemon.rs` (`startup.daemon_protocol.checked`)

**Tauri frontend events**: `sessions-updated` and `tmux-focus-changed` are emitted from `src-tauri/src/daemon_lifecycle.rs`.

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
| `just visual-shot C S [V] [T] [OUT]` | One visual-host fixture shot at a real window size (Edge headless). For viewport-anchored popups the 960x640 browser lane cannot judge. `just visual-shot-stop` stops only the server it started. |
| `just metrics` | Quality KPI snapshot (tests, coverage, build health, code size, E2E inventory). |
| `just test` | All non-E2E tests: Rust compile check + Rust unit (`--lib --bins`, heavy daemon/network/watcher suites skipped) + Rust integration/system (every `src-tauri/tests/*.rs`, then those heavy lib suites) + frontend unit. |
| `just infographics` | Regenerate documentation infographics from the manifest prompts (needs `.env`; see [`docs/operations/infographics.md`](docs/operations/infographics.md)). |
| `just infographics-dry-run` | Show which infographics are stale and what a regeneration run would cost. |
| `just test-fast` | Fast iteration lane: Rust compile check (`cargo check --tests`) + frontend unit tests. |
| `just check-quick` | Fast feedback for iteration: Rust format auto-fix (`cargo fmt`) + Rust compilation (`cargo check --tests`) + frontend typecheck + frontend unit tests. |
| `just check` | Full quality gate, run as two parallel lanes that are joined on every lane's status: `fmt`, then Rust (`lint-rust` + `test-rust`) beside frontend (`lint-frontend` + `lint-workflows` + `typecheck` + `test-frontend`). Output is tee'd to `.check-logs/` (override with `TAURHAUS_CHECK_LOG_DIR`). Team-lead serialized runs or pre-release only. |
| `just build-daemon` | Builds the WSL daemon binary (Linux target, runs in WSL2) |
| `just install-daemon` | Builds, stops a running daemon, captures its `TAURHAUS_*`/`RUST_LOG` env and CLI args from `/proc`, normalizes them to `--data-dir <dir> --port <port>` (defaults `$TAURHAUS_DATA_DIR` or `~/.local/share/com.taurhaus.dev`, port 17233), atomically swaps the binary, then restarts it detached with the same env/args. |
| `just build-mesh` | Resolves a mesh binary candidate via `scripts/resolve-mesh-binary.sh`: an explicit `MESH_BIN` is returned unchecked, otherwise the `MESH_PROJECT` workspace is rebuilt when its `git_commit` differs from the lock, otherwise a bundled/installed binary is returned unchecked. Not a lock gate on its own. |
| `just mesh-verify-lock` | The lock gate: compares the resolved binary's `version`, `protocol_version`, `schema_version`, and `git_commit` against `src-tauri/resources/mesh.lock.json`. Run by `bundle-mesh` and `install-mesh`. |
| `just update-mesh-lock VERSION [PROTOCOL] [SCHEMA] [COMMIT]` | Intentional entry point for bumping the mesh lock manifest. |
| `just bundle-mesh` | Copies mesh into `src-tauri/resources/mesh` and writes `mesh.version` / `mesh.manifest.json`. Lock-verified. |
| `just install-mesh` | Lock-verified mesh install to `~/.local/bin`. |
| `just export-agents PROJECT` | Writes the Claude role templates into `<PROJECT>/.claude/agents` via `--export-agent-definitions` on the app binary (a relative `PROJECT` resolves against the invocation directory). Only files carrying the generated marker are replaced, and one whose role left the catalog is removed; hand-written agents are reported as skipped. |
| `just analyze-compaction` | Native compaction-hook activity from current + rotated JSONL logs. |
| `just test-compaction TOOL TEAM MEMBER` | Triggers a real managed compaction and verifies native hook delivery (manual for Claude, automatic for Codex; also `test-compaction-claude` / `test-compaction-codex`). |
| `just monitor` | Unified resource monitor (live table by default). |
| `just bump VERSION` | Bump version in all files (tauri.conf.json, Cargo.toml, package.json, Cargo.lock, CHANGELOG.md) |
| `just release` | Create GitHub Release from current version. Pushes to remote, uploads artifacts. |

**E2E tests (run on target platform, NOT in WSL):**

E2E tests launch the real app binary via tauri-driver + WebDriverIO. They run on Linux only — Windows E2E is not supported (shared app data directory + tantivy index corruption makes reliable isolation impractical).

| Recipe | Where it runs | What it does |
|--------|--------------|-------------|
| `just test-e2e` | Linux (WSL) | Tier 1 E2E — safe-by-default (does not auto-run `install-daemon`), builds Linux debug binary, runs specs locally. |
| `just test-e2e-full` | Linux (WSL) | Tier 1 + Tier 2 (requires daemon running), safe-by-default daemon handling |
| `just test-e2e-spec SPEC` | Linux (WSL) | Single spec file (e.g. `just test-e2e-spec search-workflow`), safe-by-default daemon handling |
| `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex` | Linux (WSL) | Paid lane: a managed Codex member completes a bounded task through the mesh assignment contract, with the assignment's effort put into force before delivery (W4 experiment 3). Never in a suite run; see [`docs/operations/testing-guide.md`](docs/operations/testing-guide.md). |
| `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-deadline` | Linux (WSL) | Paid lane: measures active nudge suppression, one-shot nudge/stale actions, stage timeout, and session survival on a managed Codex member (W4 experiment 4). Never in a suite run; see [`docs/operations/testing-guide.md`](docs/operations/testing-guide.md). |
| `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-parallel` | Linux (WSL) | Paid lane: two managed Codex stages overlap in distinct fixture worktrees and inboxes, followed by a synthesized-summary W2 scanner-contract read-back (W4 experiment 5). Never in a suite run; see [`docs/operations/testing-guide.md`](docs/operations/testing-guide.md). |
| `just test-macos-e2e` | **macOS** via SSH | macOS E2E test suite on remote Mac Mini. |

For local runs that should rebuild/reinstall the daemon first, opt in explicitly: `E2E_INSTALL_DAEMON=1 just test-e2e`.
Every E2E worker isolates all writable product roots under its session temp directory via `HOME`, `TAURHAUS_DATA_DIR`, `TAURHAUS_CLAUDE_DIR`, `CODEX_HOME`, `GROK_HOME` and the taurhaus-only `TAURHAUS_AGY_DIR` (`e2e/helpers/workerEnv.js`); fixture path knobs `E2E_PROJECTS_DIR` and `E2E_TAURHAUS_PROJECT_PATH` live there too. Only explicitly paid specs copy `auth.json` into the otherwise empty scratch `CODEX_HOME`. Each worker also gets a private daemon port (derived from its session root, probed for availability in 20000-31999) and its own tmux *server* — `TMUX_TMPDIR` points inside the session root and an inherited `TMUX` is removed, because a client inside a tmux pane resolves the socket from `$TMUX` and ignores `TMUX_TMPDIR` (`e2e/helpers/laneTmux.js`). Teardown kills that server; nothing in a run touches port 17233 or the operator's tmux server. Process cleanup is ownership-checked: a run token is inherited by every child, and each live process is recorded as PID plus Linux `/proc` start time in a checkout-scoped ledger, so cleanup kills only identities that still match (`e2e/helpers/laneCleanup.js`); the one exception is the port-derived driver-pattern fallback for a worker's own driver ports, which is not ownership-checked. `e2e/specList.js` is the sealed spec manifest — every non-paid `e2e/specs/*.js` file must belong to a named group, and the four paid Codex lanes are never in a suite run.

Agent/team workflow rule:
- Use `just check-quick` during implementation.
- Do **NOT** run `just check` as an agent; team-lead owns serialized full-gate runs.

**Platform builds (run natively on target OS):**

| Recipe | What it does |
|--------|-------------|
| `just build-windows` | Syncs to `C:\taurhaus_build` by default (override with `TAURHAUS_WINDOWS_BUILD_DIR`), then runs the measured native Windows NSIS build via a PowerShell wrapper. |
| `just build-windows-sccache` | Same as `just build-windows`, but enables Windows-side `sccache` auto-detection for the native build. |
| `just install-windows` | Runs the latest Windows NSIS installer silently, verifies the installed exe hash against the built payload, then installs and restarts the matching WSL daemon (`_install-daemon-from-build`). `build-windows` no longer touches the live daemon — a daemon newer than the installed app is rejected on every reconnect. |
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

**Mesh gate**: every platform build verifies the mesh binary against `src-tauri/resources/mesh.lock.json` (`build-linux` depends on `bundle-daemon bundle-mesh`; the macOS recipes fail on a lock mismatch). When a release depends on a mesh change, work through "Updating the bundled mesh release" in [`CONTRIBUTING.md`](CONTRIBUTING.md) (`update-mesh-lock` → `bundle-mesh` → `mesh-verify-lock` → `install-mesh`, then commit `mesh.lock.json`, `mesh.manifest.json`, `mesh.version`) **before** the build steps above.

**Important**: The Windows exe is built **natively on Windows** via WSL2 interop (`powershell.exe -File` into the synced Windows workspace). We do NOT cross-compile from Linux. Never use `--target x86_64-pc-windows-msvc` from WSL, `cargo xwin`, or any cross-compilation approach. The `just build-windows` recipe handles everything — sync, Bun install, and the native Windows Tauri build.

**macOS**: The macOS app is built **natively on a Mac Mini** (Scaleway, arm64) via SSH. We do NOT cross-compile from Linux. The `just build-macos` recipe handles everything — rsync sync, Bun install, daemon build + codesign, and `cargo tauri build`. The Mac's PATH requires a login shell (`zsh -ilc`) for bun/cargo/homebrew.

If the build fails with "Access is denied" on the exe, the app is still running — close it first, then rebuild.

**Vitest cwd gotcha**: Vitest must run from the project root, NOT from `src-tauri/`. If `bunx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this, but if running vitest manually, always `cd` to the checkout root first.

**Manual visual review host**: `bun run dev:visual` starts the Vite visual fixture host for mocked component states. Use it for rapid layout iteration; use `just test-visual` for automated screenshot coverage. The host reads `?component=&scenario=&viewport=&theme=&chrome=0` from the URL, which is how `just visual-shot` addresses one fixture.

## Architecture Summary

- **Harness model**: Claude Code hosts Claude, the other CLIs host theirs; taurhaus coordinates from outside (tmux + mesh floor) and uses harness-native capabilities where they exist. Four harnesses are registered — `claude`, `codex`, `agy` (Antigravity CLI), `grok` (Grok CLI) — plus an `Unknown` variant that a retired persisted value decodes to. Per-tool code lives in capability slices behind `src-tauri/src/session_scanner/cli_tool.rs`; never branch on tool identity outside those slices. See `docs/architecture/harness-model.md`.
- **Accounts and usage**: per-tool providers (`src-tauri/src/session_scanner/accounts/`), project memory `pinned`/`last_used` (`project_tool_accounts`), resolution explicit → session → pin → last used → global default → base-command selector → default dir; app and managed team launches both resolve base-command aliases before selector rewriting, while an unavailable team probe launches the literal and logs `launch.base.unresolved` for that render. Managed Claude launches name the team's registry-resolved config dir; Codex and Grok launches name the member's selected config dir. Usage windows come from the daemon poller (`src-tauri/src/daemon/usage_poller.rs`) via the tool's own endpoint or command, credentials read at request time only — never logged, persisted or refreshed.
- **Storage**: SQLite (metadata, sessions, relationships) + tantivy (full-text search) + filesystem (source of truth for content)
- **Data location**: Tauri `app_data_dir()` by default; `TAURHAUS_DATA_DIR` can override for test/dev isolation
- **IPC**: Fine-grained commands — 101 in the `src-tauri/src/lib.rs` `generate_handler!` list on the default build (85 without the default `mesh-bridged-backend` feature, which gates the 16 coordination commands). One per operation; frontend fans out in parallel.
- **Daemon protocol**: `PROTOCOL_VERSION = 24` in `src-tauri/src/daemon/protocol.rs`. App and daemon must match **exactly** — the exact-version gate lives in `startup/setup.rs` and `ensure_expected_daemon_runtime` (`startup/daemon.rs`), and every reconnect path drops a mismatched daemon. `startup.daemon_protocol.checked` is a separate log line that labels only a *lower* daemon version `outdated`; anything else is `ok`, and the exact gate may already have rejected the daemon before it fires. The background bootstrap runs `ensure_bundled_daemon_installed`, so the bundled daemon auto-updates, and the install gate asks whether the installed daemon *differs* from the bundled one, in either direction — the app pins one protocol, so a newer daemon is as unusable as an older one. A mismatch a reconnect finds is therefore repaired, not looped on: `daemon_lifecycle.rs` reinstalls the bundled daemon, restarts it through the existing launcher (never env-less) and reconnects, once per mismatch episode, reported as `daemon.repair.started/completed/failed`. Bump the constant when a wire change requires the app to be rebuilt against the new daemon — a change to the `CliTool` wire vocabulary counts, because either side decodes the other's tool value as `Unknown`; purely additive methods ship without a bump and degrade to `UNKNOWN_METHOD`. History: **11** replaced the Claude-only account methods with `list_accounts`/`project_transcript` and added `refresh_usage`, **12** swapped the retired Google tool value for `agy`, **13** added `grok`, **14** retired the Codex compaction mode, **15** moved the deadline pass into the daemon, **16** moved team initialization into the daemon, **17** moved add/resume/stop into the daemon, **18** moved resume-team/reonboard into the daemon, **19** moved standalone team create/disband and roster edits into the daemon, **20** retired the stop_member wire surface, **21** moved the self-heal and effort passes into the daemon, **22** moved the final task-snapshot, live-presence, and active-project writers into the daemon and pinned the writer boundary, **23** added member account ids plus the daemon-owned team account switch contract, and **24** added per-team root authority and Claude team account switching.
- **Git**: libgit2 via `git2` crate. In-process, no CLI dependency.
- **Markdown**: Frontend rendering with Shiki (VS Code grammars). Raw text over IPC.
- **File rendering**: Classification → IPC → cache → render. See [`docs/file-rendering-pipeline.md`](docs/file-rendering-pipeline.md).
- **File watching**: `notify` + `ignore` crates. Pre-filtered by .gitignore. Git internals debounced 2s.
- **Session identity & activity**: Claude identity and busy/idle come from Claude Code's sessions registry (`<CLAUDE_CONFIG_DIR>/sessions/<pid>.json`, read under the process's own config dir, `procStart` PID-reuse guard); authoritative states skip the rchar heuristic and hysteresis. The process inventory is fail-soft — a degraded scan is inert and returns the last-good snapshot. Processes without a controlling terminal (e.g. detached `codex exec`) are not sessions. The UI derives every status from `src/lib/activitySignal.js`.
- **Tmux focus**: Foreground focus is owned by the daemon hub (`tmux list-clients` probed per cycle), travels inside the versioned session snapshot, and reaches the frontend as the `tmux-focus-changed` Tauri event. The hook → focus-file → inotify chain was deleted; `get_foreground_project` is only the startup IPC fallback.
- **Model & reasoning effort**: Separate fields end to end — `ModelSpec { model, reasoning_effort }` (legacy `"gpt-5.4 high"` spellings still parsed), persisted per member, rendered per CLI by `LaunchSpec::render()`. The backend `ModelCatalog` (per-model efforts, deprecation hints) and `CliVersions` gates travel on `TerminalPlatformContract` in settings; the frontend uses one `ModelSelect`. Every accepted flag spelling is registry data: the primary model flag is `--model` for Claude, Antigravity and Grok and `-m` for Codex, with `model_flag_aliases` covering the other spelling each CLI accepts (`--model` for Codex, `-m` for Grok). Effort is an `--effort` argument everywhere except Codex, which uses `-c model_reasoning_effort`; `effort_flag_aliases` adds Grok's `--reasoning-effort`. `deprecated_flags` (currently Codex's `--full-auto`) makes a base command carrying one emit `launch.flag.deprecated` without changing the render.
- **Accounts**: A capability slice, not a Claude-only path. Every registered tool has an account provider (`cli_tool.rs`, `account_provider()`); the selector is a separate capability. A tool whose registry entry names an `account_selector` — `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `GROK_HOME` — can hold several accounts and gets the chooser, chip and submenus (`account_selection: true`); Antigravity declares neither, so its provider describes the one implicit account under `~/.gemini` and no switching UI appears. Per-tool providers live in `src-tauri/src/session_scanner/accounts/`. Choices are stored in `project_tool_accounts` (migration 013, which carries the old `projects.claude_account_id` pins over) as `pinned` or `last_used`, and resolved per app launch in this order: explicit request → resumed session's transcript → project pin → last used here → global default → a selector already inside the user's base command → the tool's default dir. The resolution carries its `AccountOrigin` so the UI can say why. Detection runs in-process on Linux/macOS, in the WSL daemon on Windows. The base command is not taken literally: `session_scanner/launch_base.rs` resolves its head through the pane's own interactive shell (the tmux `default-shell`, else `$SHELL`) — in-process on Linux/macOS, through the additive `resolve_launch_base` daemon method on Windows — so an alias like `claude2` reveals the selector it carries. The commands layer performs the same cached resolution for managed team launches and carries it inward beside `account_selector_dirs`; coordination never probes the shell, idle background passes never request resolution, and a probe failure launches the configured literal with one `launch.base.unresolved` event for that render. Claude's selector is team-scoped: the daemon registry resolves one teams root for the whole team and every Claude member uses it, so mixed-account Claude teams remain impossible. Codex and Grok remain member-scoped. A head that is still not the tool's executable is reported as `launch.base.opaque` while the wrapper remains launchable.
- **Usage**: A second provider slice attached to each detected account as an in-memory snapshot (`UsageSnapshot { status, windows, note }`), polled by `src-tauri/src/daemon/usage_poller.rs`. Claude and Codex read their own OAuth usage endpoints; Antigravity runs `agy -p /usage --output-format json`; Grok has `usage: false` and the registry carries the sentence the UI shows where a meter would be. Tokens are read at request time, kept in memory, never logged, never persisted and **never refreshed** by taurhaus. The 0.6.8 Claude status-line bridge is retired: `accounts/legacy_statusline.rs` uninstalls it once per run (restoring a wrapped status line, leaving a foreign one alone) and logs `claude.usage.legacy_bridge.removed`.
- **Compaction reinjection**: `coordination/compact_hook.rs` is the only compaction owner for Claude, Codex and Grok; the tool is inferred from the reserved `GROK_*` env names grok injects into hook processes, otherwise from the transcript path. It accepts `SessionStart` with `source=compact`, plus `PostCompact` for a harness whose registry `compaction_delivery` is `MeshInbox` — grok, whose start source never reports `compact`; a `PostCompact` for a stdout-answered harness is skipped as `post_compact_signal_only`. Delivery follows the registry: `HookStdout` hands the card back as `hookSpecificOutput.additionalContext` (Claude, Codex), while `MeshInbox` queues it in the member's inbox (grok, whose passive-hook stdout is documented as ignored). grok also loads `~/.claude/settings.json` hooks, so the registry sets `compaction_hook_compat_import` and the bridge deduplicates. Managed Codex installs this hook by default when `CliVersions.codex_compaction_hooks_supported`; Codex 0.147 is the floor. Older Codex versions log `compaction.codex_hook.unsupported` once and receive no reinjection. Antigravity has no compaction hook (`compaction_hook: false`). The Codex transcript extractor/watcher/processor and its setting are retired.
- **Session handoffs**: Auto-created via Claude Code `SessionEnd` hook (agent type). Markdown + YAML frontmatter + JSON sidecar. `/handoff` skill as manual fallback.
- **Relationships**: Auto-detected from project signals (Cargo.toml deps, CLAUDE.md refs, session mentions). Opt-out, not opt-in.
- **Team templates**: Git-backed role/preset storage + `MeshTeamBuilder`-driven setup flow (quick presets, role filters, drag-and-drop roster editing) with advanced catalog/history in `TemplateBrowserPanel`, while preserving the existing initialize payload contract. Role templates are context-steering lane definitions with persisted schema fields for `focus_area`, `context_summary`, `behavior_summary`, `communication_style`, `quality_gates`, `definition_of_done`, `phase_scope`, `mode`, `inherits_from`, and `required_artifacts`, plus behavioral contract, defaults, capabilities, provenance, and constraints.
- **Mesh interop**: Team `config.json`, inbox records, and member runtime records round-trip mesh-owned fields via `#[serde(flatten)] extra` maps; runtime saves merge mesh-owned keys from the locked current file, including `appliedEffort`. Runtime decisions probe outside every lock, then commit through one per-team critical section that re-reads, compares the identity/liveness/effort dependencies, and uses the locked save; daemon interactive/background orchestrators cannot interleave a runtime-record write. The Windows app never mutates team state directly: the daemon and WSL-native hook processes are the only writers, enforced by `module_boundary_assertions`. Runtime records also carry `pane_pid`/`pane_start_time` so a reused tmux pane is detected and quarantined instead of restarting a daemon into a foreign pane, plus the launch account result so an opaque wrapper persists `accountApplied: false` and the member node says account selection is not guaranteed. The bundled mesh is version-locked (0.2.23) via `src-tauri/resources/mesh.lock.json`.
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
| `src/lib/context/` | Frontend context providers (`ProjectContext.js`, `SessionContext.js`, `ModelCatalogContext.js`). |
| `src/lib/activitySignal.js` | Single derivation of presented activity (`working`/`active`/`idle`/`uncertain`/`offline` + confidence) used by sidebar, HoverCard, and mesh canvas; a live workflow write inside 60 s reads as `working` (`workflowWriteAgeMs`). |
| `src/lib/workflowRuns.js` | Presentation shaping for workflow runs — session ordering, tree model, history row, sidebar badge, token/duration formatting. Invents none of the `null`s the scanner returns. |
| `src/lib/workflowRunStore.svelte.js` | The one poller for live runs: one shared 2 s timer, only while a watched session has an expanded live run. |
| `src/lib/modelCatalog.js` | Helpers over the backend-owned `ModelCatalog` delivered via `settings.terminal_contract.model_catalog`. |
| `src/lib/components/ModelSelect.svelte` | Effort-aware model picker used by `MeshTeamBuilder` and `RoleEditor`. |
| `src/lib/toolRegistry.js` | Frontend tool descriptors + capabilities (`FALLBACK_TOOLS` for `claude`/`codex`/`agy`/`grok`, overridden by the backend contract). |
| `src/lib/toolLogos.js` | Shared SVG logos + sidebar variants per tool, with an `unknown` fallback. |
| `src/lib/accounts.svelte.js` | Frontend per-tool account state (accounts, pins, usage, pending chooser); drives `AccountChooser.svelte` (Shell) and `AccountChip.svelte` (OverviewTab). `requestLaunch({ choose })` decides when the chooser opens: `'auto'` keeps every settled launch (named account, project memory, a resume the backend places) and interrupts only when that account's usage is spent or unauthorized (`exhaustedUsage` in `usageWindows.js`), carrying a `reason` — judged on the reading detection just returned, with a background refresh for the next launch; `'always'` is the user asking — Shift/Ctrl/Cmd+click on an Overview quick action, or `Choose account…` in a sidebar launch submenu. |
| `src/lib/usageWindows.js` | Normalisation helpers over a provider's usage windows. |
| `src/lib/components/UsageMeter.svelte` | One bar per usage window in the tool's own titles; compact form renders the weekly buckets. |
| `src/lib/HoverCard.svelte` | Sidebar hover preview focused on current activity, latest change, and relationship cues. |
| `src/lib/ContextMenu.svelte` | The app's context menu, with one level of account submenus (hover intent, grace corridor, viewport-clamped flyout). |
| `src/lib/accountMenu.js` | Registry-driven account rows for a context menu; the one place compact usage is rendered as menu text. |
| `scripts/visual-shot.sh` | Edge-headless window-size screenshot lane behind `just visual-shot`. |
| `src/lib/components/MeshTab.svelte` | Mesh orchestration state machine (gate/setup/init/runtime) |
| `src/lib/components/meshTabController.svelte.js` | Controller state/actions for `MeshTab.svelte`. |
| `src/lib/components/MeshSetupView.svelte` | Gate/empty/setup/initializing shell that hosts the primary team-builder surface. |
| `src/lib/components/MeshTeamBuilder.svelte` | Primary team setup UI with quick presets, role filters, drag-and-drop roster composition, and inline validation. |
| `src/lib/components/MeshCanvas.svelte` | Runtime node canvas that consumes `meshLayout.js` output. |
| `src/lib/components/meshLayout.js` | Pure mesh canvas layout engine for node boxes, explicit connection routes, and the run-tree child box (`RUN_TREE_METRICS`). |
| `src/lib/components/WorkflowRunTree.svelte` | A node's workflow runs drawn into the child box `meshLayout` placed: phase and agent rows while live, one line once finished. |
| `src/lib/components/WorkflowRunsPanel.svelte` | Overview-tab run history: runs across the project's known sessions, the selected run's agent table, and *Copy ledger row*. |
| `src/lib/components/MeshConnection.svelte` | SVG cubic-route renderer fed by explicit control points from `meshLayout.js`. |
| `src/lib/components/TemplateBrowserPanel.svelte` | Advanced role/preset catalog, import/export, history, and diff entry points. |
| `src/lib/components/templateBrowserController.svelte.js` | Controller state/actions for template browsing/composition. |
| `src/lib/components/TeamCustomizerPanel.svelte` | Advanced preset/draft editor used from the template catalog flow. |
| `src/lib/components/TemplateHistoryPanel.svelte` | Template commit history, diff, dirty status, and revert UI |
| `src/lib/components/templateHistoryController.svelte.js` | Controller state/actions for template history/diff/revert. |
| `src-tauri/src/startup/` | App bootstrap pipeline (`bootstrap`, `daemon`, `search`, `watchers`, `harness`, `setup`, `telemetry`, `orchestration`). |
| `src-tauri/src/session_scanner/launch.rs` | `ModelSpec { model, reasoning_effort }` (incl. `parse_legacy`) and `LaunchSpec::render()` — the per-tool launch command renderer. |
| `src-tauri/src/session_scanner/launch_base.rs` | Reads a configured base command the way the pane's interactive shell does: alias expansion behind an `AliasProbe`, opaque-head detection, per-(shell, tool, base) cache. |
| `src-tauri/src/session_scanner/shell_words.rs` | The one bounded shell-word tokenizer (quoting and escaping only — never expansion, globbing or execution) shared by `launch.rs`, `launch_base.rs`, `accounts/mod.rs` and `coordination/pipelines/helpers.rs`; `Word::assignment_name` owns the `NAME=value` quoted-span rule. |
| `src-tauri/src/commands/command_center/launching.rs` | Drives app launches through `LaunchSpec` (account resolution, `launch.command.rendered`). |
| `src-tauri/src/session_scanner/cli_tool.rs` | The harness registry: one `CliToolSpec` per tool (argv signatures, default commands, capabilities, accents, stop strategy). The only place tool identity may fan out. |
| `src-tauri/src/session_scanner/accounts/` | Tool-agnostic account/usage contracts (`mod.rs`) plus the per-tool providers `claude.rs`, `codex.rs`, `agy.rs`, `grok.rs`, and `legacy_statusline.rs` (one-shot uninstall of the 0.6.8 status-line bridge). |
| `src-tauri/src/daemon/usage_poller.rs` | Per-(tool, account) usage polling with backoff, in-flight guards, and `usage.fetched`/`usage.failed`. |
| `src-tauri/src/daemon/agy_hooks.rs` | Bounded `agy-hooks.jsonl` sink for Antigravity's busy/idle hooks. |
| `src-tauri/src/session_scanner/idle/claude_registry.rs` | Reads Claude Code's sessions registry (`<CLAUDE_CONFIG_DIR>/sessions/<pid>.json`) as authoritative identity/activity. |
| `src-tauri/src/session_scanner/idle/agy.rs` | Antigravity conversation identity from `cache/last_conversations.json` + the presence lock, falling back to the hook sink's `workspace` (newest record for the cwd whose presence lock is held) while agy has not indexed the cwd yet; hook-fed activity with a 5-minute recency bound. |
| `src-tauri/src/session_scanner/idle/grok.rs` | Grok identity from `<GROK_HOME>/active_sessions.json` and authoritative activity from the session's `events.jsonl` turn lifecycle. |
| `src-tauri/src/daemon/codex_notify.rs` | `taurhaus-daemon codex-notify <JSON>` subcommand; appends Codex turn-complete payloads to `<app_data>/codex-notify.jsonl` for the native idle edge. |
| `src-tauri/src/daemon/session_activity.rs` | Daemon session-activity hub: versioned snapshot, tmux focus, degradation cursor. |
| `src-tauri/tests/cli_renderers.rs` | Golden tests pinning `LaunchSpec::render` and `DeliveryRenderer` output, incl. the `--launch-command` / `--render-onboarding` CLI entries. |
| `src-tauri/src/services/task_query.rs` | Shared task query service for backend consumers. |
| `src-tauri/src/services/task_sync.rs` | Task synchronization service for daemon/IPC flows. |
| `src-tauri/src/daemon_api.rs` | Daemon process API wrapper used by commands/startup flows. |
| `src-tauri/src/project_provider.rs` | Active project resolution/provider utilities. |
| `src-tauri/src/provider/platform_paths.rs` | Central authority for app data, `teams_dir()`, daemon binary, log path, `codex_notify_path()`, and Claude hook paths. |
| `src-tauri/src/coordination/pipelines/` | Coordination domain pipelines (`initialize`, `members`, `effort`, `lifecycle`, `helpers`). |
| `src-tauri/src/coordination/compact_hook.rs` | One hook bridge for Claude Code, Codex and Grok (tool inferred from grok's reserved `GROK_*` hook env, else from the transcript path), with idempotent/removable Codex `hooks.json` and Grok `~/.grok/hooks` installers. Invoked via `--compact-hook`. |
| `src-tauri/src/coordination/agy_hooks_installer.rs` | Managed installer for Antigravity's activity hooks in the shared `~/.gemini/config/hooks.json` (`agy.hooks.degraded`). |
| `src-tauri/src/coordination/pipelines/effort.rs` + `task_effort.rs` | Own the assignment-effort state machine (held task, bounded stop/resume, typed outcomes) and the shared `ResumeWithFlag`/launch-rewrite policy. |
| `src-tauri/src/coordination/compaction_events.rs` | Structured delivery-result events shared by the native hook path. Hook lifecycle events are built in `compact_hook.rs`. |
| `src-tauri/src/session_scanner/transcript_boundary.rs` | Bounded transcript-tail parsing used to timestamp a Codex native-hook delivery. |
| `src-tauri/src/workflow_runs/` | Claude workflow run scanner, bounded transcript cache, live activity hint, IPC commands, and ledger-row renderer. |
| `docs/architecture/workflow-runs.md` | Workflow artifact sources, live/completed precedence, bounded reads, caches, activity semantics, and IPC platform split. |
| `src-tauri/src/templates/agent_definitions.rs` | Renders a role as a Claude Code agent definition (body from `DeliveryRenderer::render_role_sections`) and exports the Claude roles into `<project>/.claude/agents`. |
| `src-tauri/src/templates/adapters.rs` | Role import/export adapters, mapping rules, provenance, and round-trip loss tracking. |
| `src-tauri/src/templates/storage/` | Template git/storage domain split (`roles`, `presets`, `git`, `state`). |
| `scripts/build-windows.sh` | WSL-side Windows build orchestrator with measured step output. |
| `scripts/build-windows.ps1` | Native Windows build runner for `bun install` + `bun run tauri build --bundles nsis`, with optional `sccache`. |
| `scripts/windows-build-prereqs.ps1` | Native Windows prerequisite checker/installer for Bun, Rust MSVC, Visual Studio Build Tools, and NSIS. |
| `scripts/install-windows-silent.ps1` | Silent Windows installer runner with NSIS payload hash verification. |
| `docs/architecture/harness-model.md` | What taurhaus owns vs what the CLIs own: capability slices, model/effort, accounts, app↔daemon pairing, stability rules |
| `docs/coordination-architecture.md` | Coordination subsystem decisions, milestones, and status |
| `ARCHITECTURE.md` | System architecture overview and module map |
| `docs/architecture/data-architecture.md` | Authoritative map of live coordination stores, ownership boundaries, and derived state. |
| `docs/architecture/path-handling-guide.md` | Rules for root authority, normalization, and Windows/WSL/Linux path boundaries. |
| `docs/team-templates.md` | User guide for template authoring/composition/history workflows, incl. "Agent Definitions For Workflows" |
| `docs/team-delivery-standard.md` | Shared work-kind defaults, five-line assignment contract, message conventions, and result artifacts |
| `docs/design/harness-realignment-plan.md` | Harness realignment plan and implementation ledger (current PR-by-PR record) |
| `docs/archive/design/role-context-steering-review.md` | Archived: review notes for the role-system shift from capability labels to context steering |
| `docs/archive/design/agent-role-visibility.md` | Archived: mesh runtime role-visibility guidance built around focus area, context summary, and behavior boundaries |
| `docs/archive/design/sidebar-session-grouping.md` | Archived: sidebar grouping thresholds and behavior for team-linked live sessions |
| `docs/archive/design/sidebar-team-session-visuals.md` | Archived: sidebar connector-rail and stacked-logo treatment for grouped team indicators |
| `docs/operations/testing-guide.md` | Test lanes, verification gates, E2E worker isolation, paid lanes, and regression-testing rules (`docs/testing-guide.md` is a redirect stub). |
| `docs/operations/visual-testing-guide.md` | Visual testing lane boundaries, the fixture host, and screenshot conventions. |
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
| Fix session detection | `src-tauri/src/session_scanner/mod.rs`, `src-tauri/src/session_scanner/classification.rs` (authoritative vs heuristic, `activity.state.changed`), `src-tauri/src/session_scanner/scans.rs` (degraded scans return last-good), `src-tauri/src/session_scanner/idle/` (`claude_registry.rs`, `codex.rs`, `agy.rs`, `grok.rs`), `src-tauri/src/session_scanner/process.rs`, `src-tauri/src/daemon/session_activity.rs` |
| Change workflow-run UI | `src/lib/workflowRuns.js`, `src/lib/workflowRunStore.svelte.js`, `src/lib/components/WorkflowRunTree.svelte`, `src/lib/components/WorkflowRunsPanel.svelte`, `src/lib/components/meshLayout.js` (`RUN_TREE_METRICS`), `src/lib/activitySignal.js`; see `docs/architecture/workflow-runs.md` |
| Fix compaction detection / reinjection | `src-tauri/src/coordination/compact_hook.rs`, `src-tauri/src/coordination/stores/compaction.rs`, `src-tauri/src/coordination/compaction_events.rs`, `src-tauri/src/session_scanner/transcript_boundary.rs`, `src-tauri/src/commands/terminal_settings.rs` (managed Codex hook reconciliation) |
| Change task-level effort (assignment effort) | `src-tauri/src/coordination/pipelines/effort.rs` (`apply_pending_task_effort`, held-task selection and attempt budget), `src-tauri/src/coordination/task_effort.rs`, `src-tauri/src/coordination/state.rs` (`apply_task_effort_for_project`, the task-arrival trigger), `src-tauri/src/session_scanner/cli_tool.rs` (`runtime_effort`), `src-tauri/src/coordination/operational_context.rs`, `src/lib/components/meshTabUtils.js` + `MeshNode.svelte` + `MeshNodeDetail.svelte`, `src/lib/TaskBoard.svelte`; see `docs/architecture/harness-model.md` |
| Change launch command, model, or reasoning effort | `src-tauri/src/session_scanner/launch.rs`, `src-tauri/src/session_scanner/launch_base.rs` (what the pane shell makes of the base), `src-tauri/src/session_scanner/shell_words.rs` (the shared tokenizer every base parser uses), `src-tauri/src/commands/command_center/launching.rs` (app launches), `src-tauri/src/coordination/pipelines/helpers.rs` (team launches), `src-tauri/src/models/mod.rs` (`ModelCatalog`/`CliVersions`), `src/lib/modelCatalog.js` + `src/lib/components/ModelSelect.svelte`, golden tests in `src-tauri/tests/cli_renderers.rs` |
| Add or change an account/usage provider | `src-tauri/src/session_scanner/accounts/mod.rs` (the `AccountProvider`/`UsageProvider` contracts and generic resolution), then the tool's sibling module (`claude.rs`/`codex.rs`/`agy.rs`/`grok.rs`), `src-tauri/src/session_scanner/cli_tool.rs` (`account_selector`, `usage`, `usage_note`), `src-tauri/src/daemon/usage_poller.rs`, `src-tauri/src/commands/accounts/mod.rs`, `src-tauri/src/db/migrations/013_project_tool_accounts.sql`, `src-tauri/src/daemon/protocol.rs` (`list_accounts`, `project_transcript`, `refresh_usage`), `src/lib/accounts.svelte.js` + `src/lib/accountMenu.js` |
| Fix tmux focus / foreground indicator | `src-tauri/src/session_scanner/tmux.rs` (`list_clients`, `focus_from_clients`), `src-tauri/src/daemon/session_activity.rs`, `src-tauri/src/daemon_lifecycle.rs` (emits `tmux-focus-changed`), `src-tauri/src/commands/command_center/mod.rs` (startup fallback), `src/lib/shell/sessionLifecycle.svelte.js` + `src/lib/shell/events.svelte.js` |
| Change the daemon wire contract | `src-tauri/src/daemon/protocol.rs` (`PROTOCOL_VERSION` — bump when the change requires the app to be rebuilt against the new daemon; additive methods ship without a bump), `src-tauri/src/daemon_lifecycle.rs` (`classify_daemon_health`/`confirm_daemon_protocol`), `src-tauri/src/startup/daemon.rs`, then `just install-daemon` |
| Add a new CLI tool | `src-tauri/src/session_scanner/cli_tool.rs` (registry entry + argv signature + capabilities), then `session_scanner/launch.rs` (flag rendering), `session_scanner/idle/` (identity/idle source or none), `session_scanner/accounts/` (account/usage provider or none), `coordination/compact_hook.rs` (hook installer or none), `src/lib/toolRegistry.js` + `src/lib/toolLogos.js` (frontend descriptor, logo, accent), the conformance suite and `src-tauri/tests/cli_renderers.rs` goldens; `agy` (PR #39) and `grok` (PR #40) are the two worked examples. See `docs/architecture/harness-model.md` |
| Fix path/root resolution | `src-tauri/src/provider/path.rs`, `src-tauri/src/provider/platform_paths.rs` |
| Add database query logic | `src-tauri/src/db/`, then `src-tauri/src/models/mod.rs` |

## Development Workflow (Phase 5)

Workflow reference: [`CONTRIBUTING.md`](CONTRIBUTING.md) (setup and contribution flow) and the sections below.

### How Changes Are Made

Multi-agent work runs from the versioned procedures in [`.claude/workflows/`](.claude/workflows/README.md) rather than from prompts retyped each session: `feature-pr` (implement → two-lens cross-family review → fix loop → gate), `small-change` (one lens, one fix round), `fix-round` (extra rounds when a run stops short), `research-sweep` (N independent researchers), `docs-sweep` (drift sweep + claim verification). A lead starts one with `Workflow({name: "small-change", args: {worktree, branch, spec}})`, or hands a member the same call in an `ACTION REQUIRED:` notice. Sizing: a one-off edit stays inline, a contained change takes `small-change`, a PR across modules or the wire contract takes `feature-pr`. Whoever implements never reviews. `just lint` syntax-checks the scripts via `scripts/check-workflow-scripts.mjs` and proves `just check` fails on a seeded lane failure via `scripts/check-just-gates.sh`.

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
- CI runs the stable `Rust unit tests` job (`just test-rust-unit`) on every pull request, independently of the existing fast/frontend quality-gate job.
- CI runs `Rust integration tests` (`just test-rust-integration`) on every pull request, main push, and manual workflow dispatch.
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
