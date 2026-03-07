# Package Manager Recommendation (2026-03-05)

Status: Superseded (historical record only)  
Superseded by: [Bun Migration Guide (taurhaus)](../bun-migration-guide-2026-03-05.md)

This document captures an earlier recommendation in favor of pnpm. The repository has since migrated to Bun; do not use this recommendation for current operational decisions.

## Decision

**Recommend migrating from npm to pnpm** for taurhaus frontend workflows.

Rationale:
- It gives a substantial install-time win in local measurements while keeping Node ecosystem compatibility high.
- It is directly supported in our Tauri/Vite/Vitest tooling docs.
- It has lower compatibility risk than making Bun the default package manager for this repo.

## Scope and Constraints

This review covers:
1. Alternatives (pnpm, Bun, Yarn Berry)
2. Performance for taurhaus-like workflows (install, script/test startup)
3. Stack compatibility (Vite, Vitest, Svelte 5, Tauri 2, Tailwind v4)
4. Cross-platform requirements (Windows native, macOS arm64, Linux/WSL)
5. Migration effort including AI-agent behavior and enforcement cost

No code changes were made as part of this research.

## Local Benchmark Snapshot (taurhaus dependencies)

Environment:
- Node `v22.19.0`
- npm `11.6.0`
- pnpm via Corepack `10.30.3`
- Bun `1.2.20`
- Host: Linux workspace

Method notes:
- Install benchmark used a temp directory with `package.json` + `package-lock.json` copied from repo.
- Install numbers are **warm-cache** repeated installs.
- Script startup benchmark used version-only invocations to isolate runner overhead.

### 1) Install speed (warm cache)

| Command | Avg |
|---|---:|
| `npm ci --ignore-scripts --no-audit --no-fund` | **6229 ms** |
| `pnpm install --frozen-lockfile --ignore-scripts` | **2213 ms** |
| `bun install --ignore-scripts` | **1800 ms** |

Interpretation:
- pnpm was ~2.8x faster than npm in this local test.
- Bun was fastest in this local test.

### 2) Test-runner startup overhead (exec)

| Command | Avg |
|---|---:|
| `npm exec vitest -- --version` | **553 ms** |
| `pnpm exec vitest --version` | **444 ms** |
| `bunx vitest --version` | **145 ms** |

### 3) Script-path startup (`run test -- --version`)

| Command | Avg |
|---|---:|
| `npm run test -- --version` | **8150 ms** |
| `pnpm run test -- --version` | **7995 ms** |
| `bun run test -- --version` | **7643 ms** |

Interpretation:
- In full script path, differences are modest.
- Most practical gain comes from dependency install, where pnpm shows strong improvement over npm.

## Compatibility Assessment

### pnpm

Pros:
- Designed for npm compatibility while using a content-addressable store and linking for speed/space efficiency.
- Works with Corepack (`"packageManager"` field), which helps standardize versioning across contributors/agents.
- Explicitly supported in key ecosystem docs (Vite/Vitest/Tauri package pages).

Risks / gotchas:
- More strict dependency layout can expose undeclared dependency usage; pnpm documents `nodeLinker=hoisted` as a compatibility fallback.
- On Windows, pnpm documents Defender impact and recommends excluding the store path for better performance.

### Bun

Pros:
- Fastest in local install/startup tests.
- Bun package manager claims large install speedups vs npm in vendor benchmark.

Risks / gotchas:
- Lifecycle scripts for dependencies are **not run by default**; packages needing postinstall may require trusted-dependency configuration.
- For taurhaus, this is additional operational complexity/risk compared to pnpm.

### Yarn Berry

Pros:
- Mature ecosystem and flexible linkers.

Risks / gotchas:
- `node_modules` linker implementation in Berry is documented as experimental in Yarn docs, and Yarn has more configuration surface area than we need for this migration.
- Higher policy/education overhead than pnpm for this team.

## Stack Fit (Vite, Vitest, Svelte 5, Tauri 2, Tailwind v4)

- **Vite** templates/docs provide npm/pnpm/yarn/bun workflows.
- **Vitest** getting-started docs provide npm/pnpm/yarn/bun install/run examples.
- **Tauri** tooling docs/package page provide npm/pnpm/yarn/bun command equivalents for CLI usage.
- **Svelte/Tailwind** are standard Node packages in this repo; local install tests with pnpm and Bun succeeded against current dependency set.

Conclusion: pnpm is a safe compatibility target for this stack.

## Cross-Platform Fit

- **Windows native**: pnpm supports Windows and documents Defender tuning; Bun supports Windows in installation docs.
- **macOS arm64**: pnpm/Bun both support macOS.
- **Linux/WSL**: both support Linux.

Conclusion: pnpm and Bun both satisfy platform coverage; pnpm has lower behavioral risk for current workflows.

## AI-Agent Behavior and Enforcement Cost

Team-lead constraint: default agent behavior matters.

Current repo footprint tied to npm/npx references:
- `justfile`: **16** references
- `CLAUDE.md` + `docs/`: **32** references

If we migrate, agents will still often type `npm ...` by default unless enforced.

Practical enforcement package (recommended):
1. Update all `just` recipes to pnpm equivalents
2. Add `packageManager` field pinned to pnpm version
3. Update `CLAUDE.md` + onboarding/testing/build docs to “use `just` or pnpm, not npm”
4. Add a guard (preinstall check) that fails fast when using npm in this repo

Estimated migration effort:
- Mechanical command/doc updates: ~0.5-1 day
- Validation on Linux + Windows build path + macOS SSH path: ~0.5-1 day
- Total: **~1-2 engineering days**

## Recommendation Detail

Adopt **pnpm as default** for taurhaus, with explicit enforcement.

Why not “stay on npm”:
- Local install-time deltas are large enough to justify change.
- pnpm gives most of the practical speed benefit with lower compatibility risk than Bun.

Why not “Bun as default”:
- Fastest raw timings, but dependency lifecycle-script behavior introduces extra operational risk/policy burden for this repo.

## Suggested Migration Plan (Phased)

1. **Prep**
- Generate `pnpm-lock.yaml` from current dependency graph.
- Pin pnpm via `packageManager` + Corepack guidance.

2. **Recipe migration**
- Replace npm/npx usage in `justfile` with pnpm equivalents (`pnpm install`, `pnpm run`, `pnpm dlx` where needed).
- Validate Windows `cmd.exe` and macOS SSH recipe paths.

3. **Agent/document enforcement**
- Update `CLAUDE.md`, onboarding, operations docs, testing guide.
- Add explicit “do not use npm directly for this repo” guidance.
- Add optional hard guard for accidental `npm install`.

4. **Verification + fallback**
- Full CI/test run after migration.
- Keep a short rollback note: regenerate `package-lock.json`, revert recipe/docs if critical issue emerges.

## Sources

- pnpm motivation and benchmark links: https://pnpm.io/motivation
- pnpm installation + Corepack + Windows notes: https://pnpm.io/installation
- Bun package manager benchmark claim: https://bun.sh/docs/pm
- Bun install behavior (`trustedDependencies`, lockfile migration): https://bun.sh/docs/cli/install
- Node `packageManager` + Corepack docs: https://nodejs.org/api/packages.html#packagemanager
- Vite getting started (npm/yarn/pnpm/bun create flows): https://vite.dev/guide/
- Vitest getting started (npm/yarn/pnpm/bun examples): https://raw.githubusercontent.com/vitest-dev/vitest/main/docs/guide/index.md
- Tauri package commands for npm/yarn/pnpm/bun: https://www.npmjs.com/package/%40tauri-apps/cli
- Yarn Berry linkers / node-modules plugin note: https://yarnpkg.com/features/linkers
- Yarn node-modules plugin API (experimental notice): https://yarnpkg.com/api/plugin-nm
