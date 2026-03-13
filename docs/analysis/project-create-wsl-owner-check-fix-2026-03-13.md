# Project Create WSL Owner-Check Fix

**Date:** 2026-03-13  
**Task:** #1267

## Summary

One-step project creation on WSL UNC paths was still taking the host-side
libgit2 initialization path even though the rest of Taurhaus already treats WSL
UNC repositories as daemon-trust / WSL-native git territory.

That mismatch let the create flow hit the same ownership-validation boundary
that Taurhaus intentionally avoids for later WSL git operations.

The fix keeps the current create/register flow but changes the repo-init step:

- local/native paths still use local libgit2 init with `main`
- WSL UNC paths now initialize the repository inside WSL with `git init -b main`

## Exact Root Cause

`create_project_impl(...)` in `src-tauri/src/commands/projects.rs` always ran:

- `git2::Repository::init_opts(&target_dir, ...)`

for every target path, including WSL UNC targets like:

- `\\wsl.localhost\Ubuntu\home\user\projects\new-project`

That was inconsistent with Taurhaus's existing git trust boundary:

- WSL UNC paths are explicitly recognized by
  `provider::path::requires_daemon_git_trust(...)`
- later git operations on those paths avoid host-side local git access because
  owner validation can reject them

So the create flow failed before the normal WSL-safe path could take over:

- directory creation succeeded
- host-side libgit2 repo init hit the WSL ownership/trust boundary
- the one-step add flow failed even though the target path itself was valid

## What Changed

The fix stays narrow inside `src-tauri/src/commands/projects.rs`.

Changes:

- extracted `initialize_project_repo(...)`
- extracted `initialize_project_repo_with_runner(...)` for regression coverage
- WSL UNC targets now:
  - parse the distro from the UNC path
  - convert the target path to Linux form
  - run `wsl ... sh -lc 'mkdir -p "$1" && git -C "$1" init -b main'`
- non-WSL targets still use local libgit2 init with `initial_head("main")`

No global git trust was widened, and the registration/reseed path stayed
unchanged.

## Regression Coverage

Added backend regression coverage in `src-tauri/src/commands/projects.rs`:

- `initialize_project_repo_uses_wsl_runner_for_wsl_unc_targets`
- `create_project_wsl_one_step_flow_registers_after_wsl_git_init`

These prove that the WSL create branch now:

- routes through the WSL-native initializer instead of host-side libgit2
- preserves `main` branch initialization intent
- completes the create-and-register flow for the one-step path

The existing local create regression stayed green:

- `create_project_initializes_git_on_main_and_registers_project`

## End-to-End Result

Within command-layer coverage, one-step create now works end to end for the WSL
branch:

- create target directory
- initialize git via WSL-native path
- register the project successfully

That is the relevant end-to-end path for the bug, because the failure was in the
backend create command before frontend/UI state transitions diverged.

## Remaining Risk

This fix assumes `git` is available inside the target WSL distro, which matches
the rest of Taurhaus's WSL runtime expectations.

If WSL is present but `git` is missing or broken inside that distro, creation
will still fail, but now with the correct WSL-side initialization error instead
of a misleading host-side owner-check failure.
