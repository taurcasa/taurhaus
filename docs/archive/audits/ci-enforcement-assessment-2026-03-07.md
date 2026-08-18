# CI enforcement assessment

Date: 2026-03-07
Task: #573
Finding: Q-PRD-01

## Current state

- There is no repo-local CI configuration: no `.github/workflows/` and no checked-in CI pipeline file.
- There is also no active repo-managed git hook; `.git/hooks/` only contains the default sample hooks.
- The repo already has a clear local quality-gate split:
  - `just check-quick` for per-task iteration in [justfile](/home/user/projects/taurhaus/justfile:53)
  - `just check` as the full serialized gate in [justfile](/home/user/projects/taurhaus/justfile:30)
- Team workflow docs explicitly say agents should use `just check-quick` and team-lead owns serialized `just check`: [CLAUDE.md](/home/user/projects/taurhaus/CLAUDE.md:159), [CONTRIBUTING.md](/home/user/projects/taurhaus/CONTRIBUTING.md:76)

## Assessment

This is mostly audit noise in the current workflow, not an urgent gap.

Why:

1. The full gate is intentionally centralized already.
   - The repo does not use a PR-driven multi-contributor cloud review flow as the primary quality control.
   - Adding CI just to repeat the same gate after a trusted local serialized run adds delay more than protection.

2. The existing `just` lanes do not map cleanly to cloud enforcement.
   - `just check-quick` auto-runs `cargo fmt`, so it is good for local iteration but not ideal as a CI enforcement job.
   - `just check` is the real gate, but it is heavier and intentionally reserved for serialized runs.

3. Real platform builds are intentionally native and would be awkward to reproduce in generic CI.
   - Windows release builds are native-via-WSL interop.
   - macOS builds run on the remote Mac host.
   - Recreating the meaningful release path in GitHub Actions would require more infrastructure, more caching, and probably self-hosted runners before it adds real confidence.

## Cost / tradeoffs

### If we added GitHub Actions now

Minimal useful hosted lane:

- Linux-only smoke checks on push/PR:
  - `cargo fmt --check`
  - `cd src-tauri && cargo check --tests`
  - `bun run typecheck`
  - `bun run test`

What that would cost:

- modest GitHub Actions minutes for Linux-only validation
- Rust + Bun dependency cache setup and maintenance
- ongoing time spent on cache breakage and action drift

What it would not cover:

- Windows native packaging path
- macOS native packaging path
- remote-Mac build assumptions

So the first CI lane would be a smoke lane only, not actual release enforcement.

### If we added a pre-push hook instead

Pros:

- cheaper operationally
- matches the repo's current local-first workflow
- catches missed fast-lane verification before push

Cons:

- only protects machines where the hook is installed
- easier to bypass intentionally

## Recommendation

Do not add hosted repo CI yet.

Instead:

1. Keep the current serialized team-lead `just check` gate as the source of truth.
2. If stronger enforcement is wanted, add a repo-managed opt-in hook installer plus a non-mutating pre-push command, for example:
   - `cargo fmt --check`
   - `cd src-tauri && cargo check --tests`
   - `bun run typecheck`
   - `bun run test`
3. Revisit hosted CI only when one of these becomes true:
   - external PR review becomes the main merge path
   - more contributors push directly without the serialized lead gate
   - release confidence needs an independent Linux smoke signal before merge

## Effort estimate

- Pre-push enforcement lane + installer docs: small, about half a day
- Linux-only GitHub Actions smoke lane: small-to-medium, about half a day to one day
- Cross-platform meaningful CI aligned with actual release paths: not worth it right now
