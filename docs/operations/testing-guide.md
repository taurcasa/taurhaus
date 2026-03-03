# Testing guide

Testing strategy, test lanes, and procedures for the taurhaus project.

## Overview

Testing follows TDD for logic and visual review for layout. Three layers cover the stack: Rust unit tests, frontend Vitest tests, and E2E tests via WebdriverIO. The quality gate (`just check`) runs on every task.

## Philosophy

- **TDD for logic** — red, green, refactor. Write the failing test first, make it pass, clean up.
- **Visual review for layout** — UI appearance is verified visually, not through pixel-perfect assertions.
- **AC-driven coverage** — every acceptance criterion gets a test. No numeric coverage targets.
- **Regression guards** — every bug fix ships with a test that stays forever. Non-negotiable.

## Test layers

### Rust unit tests

Per-module `#[test]` functions with `pretty_assertions` for readable diffs and `tempfile` for isolated filesystem tests.

```bash
just test-rust-unit       # Unit tests (excludes daemon/network-heavy tests)
just test-rust-fast       # Compile check only (fast feedback)
just test-rust-integration # System/integration tests
```

Tests live alongside the code they test (standard Rust `#[cfg(test)] mod tests` pattern).

### Frontend unit tests

Vitest + JSDOM + `@testing-library/svelte`. Tests cover components, stores, and utility modules.

```bash
just test-frontend        # Run all frontend tests
```

**Vitest cwd gotcha**: Vitest must run from the project root (`/home/mstie/projects/taurhaus`), not from `src-tauri/`. If `npx vitest run` reports "No test files found", you're in the wrong directory. The `just test` recipe handles this automatically.

Test files follow the pattern `*.test.js` alongside the source they test (e.g., `src/lib/format.test.js`).

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
1. Ensure daemon is current: `just install-daemon`
2. Build E2E binary: `just build-e2e` (debug/no-bundle Tauri build)
3. Run tests

**Skip build** (when binary is known-fresh): `E2E_SKIP_BUILD=1 just test-e2e-spec SPEC`

Test specs live in `e2e/specs/` — 18 spec files covering all major features.

## Test lanes

| Recipe | What it runs | Speed |
|--------|-------------|-------|
| `just test` | All unit tests (Rust + frontend) | ~30s |
| `just test-rust-fast` | Cargo test compile check | ~10s |
| `just test-rust-unit` | Rust unit tests (no daemon/network) | ~15s |
| `just test-rust-integration` | System/integration tests | ~30s |
| `just test-frontend` | Vitest frontend tests | ~10s |
| `just test-e2e` | Tier 1 E2E (build + run) | ~3min |
| `just test-e2e-full` | Tier 1 + Tier 2 E2E | ~5min |
| `just test-e2e-spec SPEC` | Single E2E spec | ~2min |
| `just test-macos` | Rust tests on remote Mac Mini | ~1min |
| `just test-macos-e2e` | macOS E2E on remote Mac Mini | ~5min |

### Bisection recipes

When a test failure needs narrowing down:

```bash
just test-rust-bisect-unit          # Bisect unit tests by module
just test-rust-bisect-heavy         # Bisect daemon/network tests
just test-rust-bisect-commands      # Bisect commands module
just test-rust-bisect-coordination  # Bisect coordination module
```

## Quality gate

```bash
just check   # Full quality gate
```

This runs:
1. `cargo clippy` — Rust lints
2. `npx svelte-check` — Svelte type checking
3. All tests (Rust + frontend)

**Run on every task.** The quality gate must pass before any work is considered complete.

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
| `justfile` | All test recipes and quality gate |
| `e2e/README.md` | E2E runbook and troubleshooting |
| `e2e/specs/regressions.js` | E2E regression test suite |
| `vitest.config.js` | Frontend test configuration |
| `e2e/wdio.conf.js` | WebdriverIO configuration |

## Related documents

- [CLAUDE.md](../../CLAUDE.md) — TDD policy, quality gates, regression testing rules
- [Build and release](build-and-release.md) — build recipes and release workflow
