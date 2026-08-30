# Testing guide

Testing strategy, test lanes, and procedures for the taurhaus project.

## Overview

Testing follows TDD for logic and visual review for layout. The maintained lanes are Rust tests, frontend Vitest tests, browser-mode visual tests, and E2E tests via WebdriverIO. The per-task verification gate is `just check-quick`.

## Philosophy

- **TDD for logic** — red, green, refactor. Write the failing test first, make it pass, clean up.
- **Visual review for layout** — UI appearance is verified visually, not through pixel-perfect assertions.
- **AC-driven coverage** — every acceptance criterion gets a test. No numeric coverage targets.
- **Regression guards** — every bug fix ships with a test that stays forever. Non-negotiable.

## Test layers

### Rust tests

Per-module `#[test]` functions with `pretty_assertions` for readable diffs and `tempfile` for isolated filesystem tests.

```bash
just test-rust            # Full Rust lane (fast compile + unit + integration/system)
just test-rust-fast       # Compile check only (fast feedback)
just test-rust-unit       # Unit/bin tests, heavy suites excluded
just test-rust-integration # Serialized integration/system suites
just test-daemon-connectivity # Manual daemon chain verification (WSL/local)
```

Test placement follows two patterns:

- command-layer modules keep external sibling `tests.rs` files
- lower-level modules keep inline `#[cfg(test)] mod tests`

### Frontend unit tests

Vitest + JSDOM + `@testing-library/svelte`. Tests cover components, stores, and utility modules.

```bash
just test-frontend        # Run all frontend Vitest tests
just test-visual          # Run browser-mode visual screenshot tests
```

#### Full-window screenshots (`just visual-shot`)

`just test-visual` renders a component into a 960×640 test page. A popup that
positions itself against the *viewport* — the account chooser overlay, the
account chip's menu, the context menu and its submenus — cannot be judged
there: it needs a real window at a real size, with the app's own frame markup
around it.

```bash
just visual-shot shell-popups chooser-light laptop light        # prints the PNG's WSL path
just visual-shot shell-popups chip-menu-dark narrow dark shot   # custom output name
just visual-shot-stop                                           # stop the server it started
```

- Starts the visual host on port 5211 (`--strictPort`) **only if nothing is
  already listening there**, and `visual-shot-stop` kills only a pid it wrote
  down and re-verified with `ps`. Somebody else's `bun run dev:visual` is never
  touched.
- Shoots with Windows Edge headless (`msedge.exe --headless=new --screenshot`)
  against `http://localhost:5211/?component=…&scenario=…&viewport=…&theme=…&chrome=0`.
  `VISUAL_SHOT_EDGE`, `VISUAL_SHOT_PORT`, and `VISUAL_SHOT_WINDOWS_DIR` override
  the browser, port, and output directory.
- The URL is the fixture's address: the visual host reads `component`,
  `scenario`, `viewport`, and `theme` from `location.search`, and `chrome=0`
  drops its own controls so the shot is the fixture alone at window size.
- Viewports are the host's presets: `desktop` (1920×1080), `laptop` (1366×768),
  `narrow` (1024×768); themes are `light` and `dark` and nothing else (`exit 2`
  — the host falls back for a theme it does not know, so an accepted `drak`
  would file the scenario's own theme under that name).
- A shot is evidence, so every way of producing an irrelevant one fails instead:
  the listener on the port must identify itself as the visual host (`exit 6`),
  the page must report the state that was asked for — component, scenario,
  viewport and theme, written into `data-visual-host-fixture` and read back from
  the same Edge run's DOM dump (`exit 7`, the usual cause being a mistyped
  component or scenario; matched as a fixed string, so a name carrying `.` or
  `*` cannot match the host's fallback), the file must be a PNG whose IHDR says
  exactly the viewport preset's pixels (`exit 10` — the run forces
  `--force-device-scale-factor=1`, so a shot that comes back another size was
  rendered at another window size), Edge's exit status counts (`exit 8`), and
  the browser runs under a wall clock that insists: TERM, then KILL (`exit 9`,
  `VISUAL_SHOT_TIMEOUT_S` default 90 s, `VISUAL_SHOT_KILL_AFTER_S` default 5 s).
- PNGs land in `C:\taurhaus_build\shots` and are **not** committed — `*.png` is
  gitignored outside `docs/`. Paste them into the PR description as before/after
  evidence.

**Vitest cwd gotcha**: Vitest must run from the project root (`/home/user/projects/taurhaus`), not from `src-tauri/`. If `bunx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this automatically.

Test files follow the pattern `*.test.js` alongside the source they test (e.g., `src/lib/format.test.js`).

For manual visual review, run `bun run dev:visual` and use the fixture host documented in [`visual-testing-guide.md`](./visual-testing-guide.md).

### E2E tests

WebdriverIO + `tauri-driver`. E2E tests launch the real app binary and interact with it through the accessibility tree. Linux only — Windows E2E is not supported due to shared app data directory conflicts.

```bash
just test-e2e             # Tier 1 — basic specs (no daemon required)
just test-e2e-full        # Tier 1 + Tier 2 (requires running daemon)
just test-e2e-spec SPEC   # Single spec file (e.g., just test-e2e-spec search-workflow)
just test-macos-e2e       # macOS E2E via SSH on remote Mac Mini
```

**Tiers**:
- **Tier 1**: Tests that work without a daemon connection (UI, navigation, settings)
- **Tier 2**: Tests requiring a running daemon (session detection, file watching, command center)

**E2E setup** (see [e2e/README.md](../../e2e/README.md) for troubleshooting):
1. By default the recipes do **not** reinstall the daemon. Opt in only if needed: `E2E_INSTALL_DAEMON=1 just test-e2e`
2. The recipes build the E2E binary automatically unless `E2E_SKIP_BUILD=1` is set
3. Run the tier/spec command you need

**Skip build** (when binary is known-fresh): `E2E_SKIP_BUILD=1 just test-e2e-spec SPEC`

Test specs live in `e2e/specs/` and are split by workflow/domain rather than by one monolithic suite.

#### Paid E2E lanes

Two specs drive a real Codex subscription and cost money every time they run. `e2e/specList.js` keeps both out of the config's spec list, so no suite run — including a bare `bunx wdio run e2e/wdio.conf.js` — picks them up; each is started by name and nothing else starts it.

| Lane | Recipe | What it proves |
|---|---|---|
| `compaction-codex-hooks` | `E2E_INSTALL_DAEMON=1 just test-e2e-spec compaction-codex-hooks` | A managed Codex member gets its restored-context card back through the native hook bridge. See [compaction-testing.md](compaction-testing.md). |
| `managed-stage-codex` | `E2E_INSTALL_DAEMON=0 just test-e2e-spec managed-stage-codex` | A managed Codex member completes a bounded task through the mesh assignment contract, with the assignment's effort put into force before the notice is delivered (W4 experiment 3). |

Both run against isolated roots — `TAURHAUS_DATA_DIR`, `TAURHAUS_CLAUDE_DIR` and a scratch `CODEX_HOME` holding only a copy of `auth.json` plus a generated `config.toml`. The operator's `~/.codex` is read once at copy time and never written; `~/.claude` is neither read nor written. Naming either lane on the command line is what tells `wdio.conf.js` to build that scratch Codex home.

`managed-stage-codex` additionally sets `CLAUDE_DIR` on the panes it creates, because its member runs `mesh` itself: taurhaus passes `--claude-dir` to the member *daemon* it spawns but exports no Claude root into the pane, so without it the member's own `mesh send` would bootstrap the run's team inside the operator's real home. Its team lead is a Claude identity and an inbox, not a working agent — it is launched into the isolated, credential-free `CLAUDE_CONFIG_DIR` and never takes a turn, so the lane spends nothing on Claude. Measured cost and wall clock: [w4-experiment-3.md](../design/research/w4-experiment-3.md).

Both lanes take on every host change they make as an undo (`e2e/helpers/laneCleanup.js`) that runs on interrupt as well as on teardown, and both restore the `taurhaus` tmux session's environment the moment the pane-creating call returns. They differ in whose tmux server that session lives on. `compaction-codex-hooks` uses the operator's, and kills the panes it opened — identified by a working directory inside the session temp root. `managed-stage-codex` runs against a tmux server of its own: `wdio.conf.js` points `TMUX_TMPDIR` at a directory inside the session temp root and clears an inherited `TMUX` before starting tauri-driver, so the app under test and every daemon it spawns create their panes there. That lane refuses to start unless both it and the app are on that server (checked against the app's own `/proc/<pid>/environ`), and teardown takes the whole server down rather than guessing which panes were its own — so a `set-environment` carrying this run's temporary roots can never reach a pane the operator opens.

## Test lanes

| Recipe | What it runs |
|--------|-------------|
| `just test` | All non-E2E tests (Rust + frontend) |
| `just test-fast` | Rust compile-check + frontend Vitest |
| `just test-rust-fast` | Cargo test compile check |
| `just test-rust-unit` | Rust unit tests (no daemon/network) |
| `just test-rust-integration` | System/integration tests |
| `just test-frontend` | Vitest frontend tests |
| `just test-visual` | Browser-mode visual screenshot lane |
| `just visual-shot C S [V] [T] [OUT]` | One fixture shot at window size via Edge headless |
| `just visual-shot-stop` | Stop the visual host `visual-shot` started |
| `just test-daemon-connectivity` | Manual daemon connectivity chain checks |
| `just test-e2e` | Tier 1 E2E |
| `just test-e2e-full` | Tier 1 + Tier 2 E2E |
| `just test-e2e-spec SPEC` | Single E2E spec |
| `just test-e2e-spec compaction-codex-hooks` | Paid Codex compaction lane (never in a suite run) |
| `just test-e2e-spec managed-stage-codex` | Paid managed Codex stage lane (never in a suite run) |
| `just test-macos` | Rust tests on remote Mac Mini |
| `just test-macos-e2e` | macOS E2E on remote Mac Mini |
| `just agent-quality` | Agent-facing wrapper around `just check-quick` |

### Bisection recipes

When a test failure needs narrowing down:

```bash
just test-rust-bisect-unit          # Bisect unit tests by module
just test-rust-bisect-heavy         # Bisect daemon/network tests
just test-rust-bisect-commands      # Bisect commands module
just test-rust-bisect-coordination  # Bisect coordination module
```

## Verification gates

```bash
just check-quick   # Per-task fast gate
just check         # Full gate (team-lead serialized runs or pre-release)
```

`just check-quick` runs:
1. `cargo fmt` — Rust format auto-fix
2. `cargo check --tests` — Rust compile + test-target validation
3. `bun run typecheck` — Svelte type checking
4. `bun run test` — Frontend unit tests

`just agent-quality` delegates to `just check-quick` and exists as the explicit pre-completion gate for agent workflows.

`just check` runs the full gate:
1. `cargo fmt --check` via `just fmt` — Rust formatting enforcement
2. `cargo clippy` — Rust lints
3. `bun run lint` — frontend lint
4. `bun run typecheck` — Svelte type checking
5. All non-E2E tests via `just test`

**Run `just check-quick` on every task.** In team/agent workflows, agents should not run `just check`; team-lead owns serialized full-gate runs.

E2E tests run at milestones, not on every task.

## Regression testing

Every regression fix ships with a corresponding test. This is non-negotiable.

### Where regression tests go

| Layer | Location | Format |
|-------|----------|--------|
| E2E | `e2e/specs/regressions.js` | One `describe` block per regression |
| Rust | Affected module's `#[cfg(test)]` | `#[test]` with `// Regression:` comment |
| Frontend | Affected module's `.test.js` | Test case with `// Regression:` comment |

### What to document

Every regression test must include:
1. **What broke** — the visible symptom
2. **Which commit broke it** — the offending change
3. **Why** — root cause explanation

Example:
```rust
#[test]
fn session_file_dedup_rejects_duplicate_path() {
    // Regression: duplicate session imports caused sidebar duplication
    // Commit: abc1234 — removed unique index during migration refactor
    // Root cause: migration 002 was skipped when running from clean DB
    ...
}
```

## Visual review

Frontend tasks undergo visual review using 8 categories, each scored 1–10 with a minimum of 9 per category.

**Dual review process**:
1. Self-review by the implementer
2. Cross-review by the other model family (screenshot analysis), the same Opus ↔ Codex pairing every PR review loop uses
3. Lower score wins; the orchestrator is final arbiter with justified override

This applies to frontend tasks only — backend tasks skip visual review.

## Key files

| File | Purpose |
|------|---------|
| `justfile` | All test recipes and verification gates |
| `e2e/README.md` | E2E runbook and troubleshooting |
| `e2e/specs/regressions.js` | E2E regression test suite |
| `vitest.config.ts` | Frontend unit test configuration |
| `vitest.visual.config.js` | Browser-mode visual test configuration |
| `scripts/visual-shot.sh` | Edge-headless window-size screenshot lane |
| `src/visual-host/query.js` | URL → fixture address for the visual host |
| `e2e/wdio.conf.js` | WebdriverIO configuration |
| `scripts/rust-test-bisect.sh` | Rust lane/module bisect helper |

## Related documents

- [CLAUDE.md](../../CLAUDE.md) — TDD policy, quality gates, regression testing rules
- [visual-testing-guide.md](./visual-testing-guide.md) — manual visual host and screenshot lane details
- [Build and release](build-and-release.md) — build recipes and release workflow
