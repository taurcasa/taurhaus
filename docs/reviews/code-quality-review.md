# Code Quality Review

Date: 2026-03-28

Scope:
- Rust backend command layer in `src-tauri/src/commands/`
- Core frontend Svelte surfaces in `src/` and `src/lib/components/`
- Validation signals from `cargo check --tests` and `bunx vitest run`

Methods:
- Read `CLAUDE.md` and compared current code against repo standards
- Audited the IPC command layer first, then sampled major frontend surfaces
- Checked security-sensitive file access paths
- Ran `cargo check --tests` in `src-tauri/`
- Ran `bunx vitest run` from the repo root

## Executive Summary

The codebase has strong foundations in a few key places: Rust compiles cleanly, IPC lifecycle instrumentation is present, and the file-access paths reviewed have explicit traversal protections. The main quality issues are not correctness failures in core backend logic. They are consistency failures and maintenance drift:

- the backend now exposes two IPC error contracts at once
- several foreground daemon-backed commands behave inconsistently under load
- frontend theme logic has drifted back into templates despite the `$derived` token rule
- some major Svelte surfaces remain too large and state-heavy to review or change safely
- the frontend unit-test lane is currently broken in this environment, which weakens regression protection

## Findings

### 1. Mixed IPC error contracts across the command layer

- Category: inconsistency
- Severity: high
- Evidence:
  - Newer command modules use typed IPC errors, for example `src-tauri/src/commands/tasks.rs:57` and `src-tauri/src/commands/coordination.rs:77`.
  - Older modules still return raw strings, for example `src-tauri/src/commands/projects.rs:105`, `src-tauri/src/commands/command_center/mod.rs:43`, `src-tauri/src/commands/files.rs:118`, and `src-tauri/src/commands/git.rs:129`.
  - The repo already has a structured error model in `src-tauri/src/errors.rs:31`.
- Why this matters:
  - Frontend callers have to tolerate both plain string failures and structured `IpcError` responses.
  - Retryability, command attribution, and machine-readable error codes are only available on part of the surface.
  - This makes cross-command UX inconsistent and blocks systematic error handling in the frontend.
- Recommended fix:
  - Treat `IpcResult<T>` as the command-layer standard for all Tauri commands.
  - Migrate one domain at a time, starting with `projects`, `files`, `git`, and `command_center`.
  - Use `CommandResultExt::ipc_cmd(...)` so every command emits consistent `code`, `retryable`, and `command` fields.

### 2. Frontend unit-test lane is effectively broken

- Category: bug
- Severity: high
- Evidence:
  - `cargo check --tests` completed successfully in `src-tauri/`.
  - `bunx vitest run` failed with 89 worker-start errors.
  - The repeated failure was `ERR_REQUIRE_ESM`, specifically `html-encoding-sniffer` requiring `@exodus/bytes/encoding-lite.js`.
- Why this matters:
  - The repo appears to have broad frontend test coverage, but that coverage is not currently runnable.
  - Regressions in large surfaces like `Shell`, `FilesTab`, `GitTab`, and mesh components are no longer being screened by the declared unit-test lane.
  - This is a quality-gate problem, not just local developer inconvenience.
- Recommended fix:
  - Fix the dependency boundary in the Vitest/JSDOM environment first, before adding more frontend tests.
  - Pin or replace the transitive package combination that triggers the CJS-to-ESM failure.
  - Add a lightweight CI smoke step that runs a tiny representative JSDOM spec so this breaks loudly and early.

### 3. Busy-daemon fail-fast behavior is applied inconsistently

- Category: inconsistency
- Severity: medium
- Evidence:
  - `src-tauri/src/commands/git.rs:138` rejects recent-commit loads when the shared daemon lane is busy.
  - `src-tauri/src/commands/files.rs:166` does the same for README loads.
  - Similar foreground reads do not use the same guard:
    - `src-tauri/src/commands/git.rs:158`
    - `src-tauri/src/commands/git.rs:176`
    - `src-tauri/src/commands/files.rs:126`
    - `src-tauri/src/commands/files.rs:210`
- Why this matters:
  - Similar user actions can degrade in totally different ways under the same daemon-pressure condition.
  - Some screens fail fast with a recoverable message, while others can block on the same shared transport.
  - That breaks the repo's "snappy" interaction goal and makes performance bugs hard to reason about.
- Recommended fix:
  - Decide whether all foreground daemon reads should fail fast or queue.
  - Encode that policy once in a shared helper used by all relevant file/git commands.
  - Cover the policy with command-layer tests so future handlers do not diverge.

### 4. Dark-mode token discipline is not being followed in templates

- Category: inconsistency
- Severity: medium
- Evidence:
  - `CLAUDE.md` requires dark mode via named `$derived` tokens and explicitly says not to inline color ternaries in templates.
  - There are 358 `dark ? ... : ...` occurrences in `.svelte` files.
  - Representative user-facing examples:
    - `src/lib/FirstRunWizard.svelte:264`
    - `src/lib/FirstRunWizard.svelte:274`
    - `src/lib/FilesTab.svelte:509`
    - `src/lib/FilesTab.svelte:516`
    - `src/lib/GitTab.svelte:620`
    - `src/lib/GitTab.svelte:648`
- Why this matters:
  - Theme logic is split between token definitions and template branches.
  - Theming becomes harder to audit, and component markup gets noisier than it needs to be.
  - This is exactly the sort of standards drift that accumulates into visual inconsistency.
- Recommended fix:
  - Treat inline `dark ?` template branches as debt to retire when touching a component.
  - For each component, move color/variant selection into named `$derived` values near the top of the script block.
  - Add a lint check or repo-local script that flags new template-level dark-mode ternaries outside approved visual-host fixtures.

### 5. A few core Svelte surfaces are too large and state-dense

- Category: debt
- Severity: medium
- Evidence:
  - `src/lib/components/MeshTeamBuilder.svelte` is 2696 lines and declares 39 local `$state` values.
  - `src/Shell.svelte` is 590 lines and declares 46 local `$state` values.
  - `src/lib/FilesTab.svelte` and `src/lib/GitTab.svelte` are both over 600 lines.
- Why this matters:
  - Reviewability is poor. A single change can affect state, side effects, theme handling, and DOM structure in the same file.
  - The controller extraction pattern exists in parts of the codebase, but these surfaces still carry too much orchestration locally.
  - Large stateful Svelte files are a common source of accidental regressions, especially while the frontend test lane is degraded.
- Recommended fix:
  - Continue the existing controller/view split instead of introducing a new abstraction style.
  - For `Shell.svelte`, move orchestration-only logic into `src/lib/shell/*` helpers until the component mainly wires semantic layout and props.
  - For `MeshTeamBuilder.svelte`, split catalog, roster, and role-editor workflows into smaller components backed by the existing helper modules.

### 6. Command-layer test placement is still inconsistent with repo standards

- Category: inconsistency
- Severity: low
- Evidence:
  - `CLAUDE.md` says command-layer modules should use an external sibling `tests.rs`.
  - `src-tauri/src/commands/command_center/mod.rs:409` follows that rule.
  - Many command modules still keep inline tests, for example:
    - `src-tauri/src/commands/files.rs:244`
    - `src-tauri/src/commands/git.rs:202`
    - `src-tauri/src/commands/projects.rs:1047`
    - `src-tauri/src/commands/settings.rs:95`
    - `src-tauri/src/commands/search.rs:103`
    - `src-tauri/src/commands/tasks.rs:468`
- Why this matters:
  - The command modules are already large; inline tests make them harder to scan.
  - Test organization is now inconsistent enough that contributors cannot rely on one command-layer pattern.
- Recommended fix:
  - Use external `tests.rs` for command modules going forward.
  - Move the largest inline test blocks first: `projects`, `files`, `git`, `daemon`, `mesh`.
  - Do not mass-migrate every command file in one change; use opportunistic cleanup while touching a module.

## Security Review Notes

No high-confidence OWASP-style vulnerability stood out in the file-access paths reviewed.

Positive signals:
- `src-tauri/src/fs/reader.rs:13` rejects absolute paths, parent traversal, and symlink escapes.
- `src-tauri/src/provider/local.rs:111` repeats traversal protection for asset reads and enforces a size cap.
- `src-tauri/src/provider/local.rs:338` includes regression coverage for traversal rejection.
- `src-tauri/src/daemon/launcher.rs:53` documents and validates distro input before shell-mediated WSL operations.

Residual risk:
- The command layer relies on downstream providers for some safety properties, which is acceptable but easy to lose track of. If command handlers continue diverging in behavior, security-sensitive checks can also drift over time.

## Dead Code / Hotspot Notes

- `cargo check --tests` did not surface obvious dead-code warnings in the reviewed Rust surfaces.
- I did not find a high-confidence unused module or function that can be deleted immediately without deeper ownership context.
- The more credible "dead weight" issue is complexity concentration rather than dead code:
  - `src-tauri/src/commands/projects.rs`
  - `src-tauri/src/commands/coordination.rs`
  - `src/lib/components/MeshTeamBuilder.svelte`
  - `src/Shell.svelte`

## Recommended Next Steps

1. Unblock the frontend test lane so regression protection is real again.
2. Standardize on `IpcResult<T>` for all command modules that still return raw `String`.
3. Normalize the daemon-busy policy across all foreground git/file reads.
4. Stop adding template-level `dark ?` branches and retire existing ones during normal feature work.
5. Split `MeshTeamBuilder.svelte` and continue shrinking `Shell.svelte` using the existing controller pattern.

## Validation Evidence

- `cargo check --tests` in `src-tauri/`: passed
- `bunx vitest run` in repo root: failed due to repeated `ERR_REQUIRE_ESM` worker startup errors involving `html-encoding-sniffer` and `@exodus/bytes/encoding-lite.js`
