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
| `just test-daemon-connectivity` | Manual daemon connectivity chain checks |
| `just test-e2e` | Tier 1 E2E |
| `just test-e2e-full` | Tier 1 + Tier 2 E2E |
| `just test-e2e-spec SPEC` | Single E2E spec |
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
2. Cross-review by Gemini Pro 3 (screenshot analysis)
3. Lower score wins; Claude is final arbiter with justified override

This applies to frontend tasks only — backend tasks skip visual review.

## Key files

| File | Purpose |
|------|---------|
| `justfile` | All test recipes and verification gates |
| `e2e/README.md` | E2E runbook and troubleshooting |
| `e2e/specs/regressions.js` | E2E regression test suite |
| `vitest.config.ts` | Frontend unit test configuration |
| `vitest.visual.config.js` | Browser-mode visual test configuration |
| `e2e/wdio.conf.js` | WebdriverIO configuration |
| `scripts/rust-test-bisect.sh` | Rust lane/module bisect helper |

## Related documents

- [CLAUDE.md](../../CLAUDE.md) — TDD policy, quality gates, regression testing rules
- [visual-testing-guide.md](./visual-testing-guide.md) — manual visual host and screenshot lane details
- [Build and release](build-and-release.md) — build recipes and release workflow
