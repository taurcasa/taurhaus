# Bun Migration Guide (taurhaus)

Date: 2026-03-05  
Status: Complete (migration implemented)

## Summary

Team decision is to migrate frontend/package-manager workflows from npm/npx to Bun for speed.  
This guide covers lockfile migration, trusted dependencies, recipe/doc updates, agent enforcement, E2E compatibility, rollback, and verification.

Scope principle: use Bun everywhere it is reasonable for JS tooling (`bun install`, `bun run`, `bunx`) while keeping Rust/Cargo/Tauri-native behavior unchanged.

## 1) Lockfile Migration Plan (`package-lock.json` -> `bun.lock`)

1. Create migration branch and keep `package-lock.json` in the first Bun PR for diff/review.
2. Run lockfile migration without install:
   - `bun pm migrate`
   - Expected: `bun.lock` created and existing lockfile preserved.
3. Install from new lockfile:
   - `bun install --frozen-lockfile`
4. Verify dependency/runtime equivalence:
   - `bun pm ls --all > /tmp/bun-tree.txt`
   - Smoke all critical paths: `just check-quick`, `just test-e2e`, `just build-e2e`.
5. After verification, remove `package-lock.json` in follow-up commit/PR.

Notes:
- `bun pm migrate` was validated locally against taurhaus lockfile in a temp workspace.
- Bun docs state automatic migration also occurs when running `bun install` with no existing `bun.lock`.

## 2) `trustedDependencies` Plan (3 approved packages)

Approved list:
- `esbuild`
- `edgedriver`
- `geckodriver`

Recommended configuration:

```json
{
  "trustedDependencies": ["esbuild", "edgedriver", "geckodriver"]
}
```

Why explicit list instead of only `bun pm trust`:
- `bun pm trust` is environment-dependent and only adds currently untrusted packages.
- Explicit list keeps cross-platform behavior deterministic (Linux/Windows/macOS).

Operational commands:
- Discover blocked scripts: `bun pm untrusted`
- Trust discovered deps interactively: `bun pm trust <name...>`

## 3) `justfile` Migration Map (16 references)

| Line | Current | Bun replacement |
|---|---|---|
| 15 | `npm run dev` | `bun run dev` |
| 24 | `npm run dev:tauri` | `bun run dev:tauri` |
| 49 | `npm run lint` | `bun run lint` |
| 53 | `npm run typecheck` | `bun run typecheck` |
| 116 | `npm run test` | `bun run test` |
| 120 | `npm run test:watch` | `bun run test:watch` |
| 127 | `npx tauri build --debug --no-bundle` | `bunx tauri build --debug --no-bundle` |
| 134 | `npx wdio run ...` | `bunx wdio run ...` |
| 140 | `npx wdio run ...` | `bunx wdio run ...` |
| 147 | `npx wdio run ...` | `bunx wdio run ...` |
| 171 | `npm run tauri build` | `bun run tauri build` |
| 321 | `cmd.exe ... npm install` | `cmd.exe ... bun install --frozen-lockfile` |
| 365 | `ssh ... npm install` | `ssh ... bun install --frozen-lockfile` |
| 419 | `ssh ... npm install` | `ssh ... bun install --frozen-lockfile` |
| 435 | `ssh ... npm run build && cargo ...` | `ssh ... bun run build && cargo ...` |
| 461 | `ssh ... npm install` | `ssh ... bun install --frozen-lockfile` |

## 4) Cross-Platform Build Path Plan

### Linux / WSL dev path
- Install Bun (if missing): `curl -fsSL https://bun.com/install | bash`
- Verify: `bun --version`
- Core flows:
  - `just dev`
  - `just test`
  - `just check-quick`

### Windows native build path (`cmd.exe`)
- Install Bun on Windows (PowerShell installer from Bun docs).
- Verify from WSL interop:
  - `cmd.exe /c "where bun"`
  - `cmd.exe /c "bun --version"`
- Build path after migration:
  - `cmd.exe /c "cd /d D:\taurhaus_build && bun install --frozen-lockfile"`
  - `cmd.exe /c "cd /d D:\taurhaus_build && cargo tauri build --bundles nsis"`

### macOS SSH build path
- Install Bun on remote Mac user once.
- Verify in login shell:
  - `ssh <host> "zsh -ilc 'bun --version'"`
- Build path after migration:
  - `ssh <host> "zsh -ilc 'cd <repo> && bun install --frozen-lockfile'"`
  - keep existing cargo/codesign steps unchanged.

## 5) CLAUDE.md + docs (32 references): what changes vs what stays

Current count (excluding `docs/audits/package-manager-recommendation-2026-03-05.md`): 32 references.

### Update (current operational docs)
- `CLAUDE.md` (4)
- `docs/operations/build-and-release.md` (6)
- `docs/operations/testing-guide.md` (2)
- `docs/e2e-performance-bug.md` (4)
- `docs/getting-started.md` (3, where project workflow commands are npm-specific)
- `docs/images/infographics.manifest.yaml` (1, Windows build flow text)
- `docs/audits/perf-audit-frontend.md` (1, if maintained as a living baseline doc)

### Keep as historical record (no retroactive rewrite)
- `docs/security/audit-2026-02-27.md` (7)
- `docs/security/team-lead-audit-2026-03-03.md` (2)
- `docs/security/sec-auditor-audit-2026-03-03.md` (1)
- `docs/security-audit-task56-2026-03-04.md` (1)

## 6) Agent Enforcement Plan (fresh sessions default to Bun)

### CLAUDE.md instruction changes
Add explicit rule set:
- Use `bun install`, `bun run`, `bunx` for JS workflows.
- Do not use `npm`/`npx` in this repo unless explicitly requested for debugging.
- Prefer `just` recipes when available.

### `package.json` enforcement knobs
1. Pin package manager:

```json
{
  "packageManager": "bun@1.2.20"
}
```

2. Add preinstall guard (recommended):

```json
{
  "scripts": {
    "preinstall": "node -e \"const ua=process.env.npm_config_user_agent||''; if(!ua.includes('bun/')){console.error('Use bun install (not npm/pnpm/yarn) in taurhaus'); process.exit(1)}\""
  }
}
```

Notes:
- This is a guardrail, not absolute security.
- CI should still validate lockfile + commands used in recipes.

## 7) E2E Path Compatibility Plan (WebDriverIO + tauri-driver)

Current E2E already uses local CLI packages; Bun command parity is good.

Required updates:
- `justfile` E2E recipes: `npx wdio` -> `bunx wdio`, `npx tauri` -> `bunx tauri`.
- `e2e/wdio.conf.js`: replace `spawn('npx', ['tauri', ...])` with Bun equivalent (for example `spawn('bunx', ['tauri', ...])`).

Validated locally in this workspace:
- `bunx wdio --version`
- `bunx tauri --version`
- `bunx vitest --version`
- `bunx svelte-check --version`

Guidance:
- Keep default `bunx` behavior (do not force `--bun`) for Node-oriented CLIs unless needed.

## 8) Rollback Plan

If migration causes regressions:
1. Revert migration PR(s) (recipes/docs/config changes).
2. Restore `package-lock.json` as canonical lockfile.
3. Remove Bun-specific fields/guards (`packageManager`, `trustedDependencies`, preinstall guard) if they block npm.
4. Re-run baseline npm flow:
   - `npm install`
   - `just check-quick`
   - `just test-e2e`
5. File focused follow-up issues for each regression found during Bun rollout.

## 9) Post-Migration Verification Checklist

### Local Linux/WSL
- `bun --version`
- `bun install --frozen-lockfile`
- `just check-quick`
- `just test`
- `just build-e2e`
- `just test-e2e`

### Windows native (via WSL interop)
- `cmd.exe /c "where bun"`
- `cmd.exe /c "cd /d D:\taurhaus_build && bun install --frozen-lockfile"`
- `just build-windows`

### macOS remote
- `ssh <host> "zsh -ilc 'bun --version'"`
- `just build-macos`
- `just test-macos`

### Lockfile/consistency
- `git diff -- package-lock.json bun.lock package.json justfile`
- Ensure no recipe path still shells out to `npm`/`npx` for project-local JS tooling.

## Additional Surfaces Outside Requested 16/32 Counts (completed)

These were not part of the “16 justfile + 32 docs” counts and were tracked as follow-on migration items. They are now completed:
- `package.json`: `lint`/`typecheck` no longer use npm wrappers.
- `src-tauri/tauri.conf.json`: `beforeDevCommand` and `beforeBuildCommand` no longer use npm.
- `e2e/wdio.conf.js`: no longer spawns `npx tauri ...`.
- `scripts/metrics.sh`: no longer uses `npx`/`npm`.
- Root docs outside `docs/` (`README.md`, `ARCHITECTURE.md`, `CONTRIBUTING.md`) were updated to Bun-era commands.

## References

- Bun install command docs (`bun install`):  
  https://raw.githubusercontent.com/oven-sh/bun/main/docs/pm/cli/install.mdx
- Bun lifecycle + `trustedDependencies`:  
  https://raw.githubusercontent.com/oven-sh/bun/main/docs/pm/lifecycle.mdx
- Bun lockfile + automatic lockfile migration:  
  https://raw.githubusercontent.com/oven-sh/bun/main/docs/pm/lockfile.mdx
- Bun installation (Linux/macOS/Windows):  
  https://raw.githubusercontent.com/oven-sh/bun/main/docs/installation.mdx
- Bun `bunx` (`npx` equivalent):  
  https://raw.githubusercontent.com/oven-sh/bun/main/docs/pm/bunx.mdx
- Tauri CLI package readme (supports npm/pnpm/yarn/bun usage):  
  `npm view @tauri-apps/cli readme`
