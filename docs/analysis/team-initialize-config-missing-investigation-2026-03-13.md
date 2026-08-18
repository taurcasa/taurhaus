# Team Initialize Config Missing Investigation

## Task

Investigate the failed team initialization for `espn_fantasy-team`, using:

- the production Windows Taurhaus log
- the WSL and Windows team directory state
- path-resolution handling across Windows, WSL Linux, and WSL UNC paths

## Production evidence

The relevant production log is:

- `C:\Users\user\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- WSL path: `/mnt/c/Users/user/AppData/Roaming/com.taurhaus.dev/taurhaus.log.jsonl`

Relevant events:

- `2026-03-13T17:16:07.980Z`
  - `coordination_get_live_team_status`
  - error: `team config not found for 'espn_fantasy-team' at \\wsl.localhost\Ubuntu\home\user\.claude\teams\espn_fantasy-team\config.json`
- `2026-03-13T17:17:16.286Z`
  - `coordination_initialize_team` received
- `2026-03-13T17:17:23.117Z`
  - `coordination_initialize_team` failed
  - same error string:
    - `team config not found for 'espn_fantasy-team' at \\wsl.localhost\Ubuntu\home\user\.claude\teams\espn_fantasy-team\config.json`

## Team directory state

At the time of investigation:

- no `espn_fantasy-team` files exist under WSL:
  - `~/.claude/teams/espn_fantasy-team`
- no `espn_fantasy-team` files exist under Windows-visible `.claude/teams`
- only the production log path was present under the Windows app data root

So the failure is not caused by a corrupt leftover `espn_fantasy-team/config.json` in the current live roots. The team directory is absent after the failed initialize attempt.

## Path-resolution findings

### What is consistent

The team-state root is consistently resolved to the Windows UNC path on Windows:

- `\\wsl.localhost\Ubuntu\home\user\.claude\teams`

That comes from `resolve_windows_mesh_teams_dir()` and is used by `CoordinationState::default_teams_dir()`.

For project references inside initialize requests, Taurhaus normalizes project paths to Linux form:

- Windows drive path or WSL UNC path -> Linux `/home/...` or `/mnt/...` form

This happens in `resolve_project_reference(...)` through `crate::provider::path::to_linux(...)`.

### What is ruled out

I did **not** find evidence that the failing `config.json` lookup itself is bouncing between:

- `\\wsl.localhost\...`
- `/home/...`
- some separate Windows home path

The production error consistently points to the same UNC team root, and the command-layer code uses that same teams root for the later sync/read operations.

So for this specific error, a direct Windows-vs-WSL-vs-UNC mismatch on the final team config path is **not** the primary finding.

## Actual defect found

The visible production error is being **masked** by a command-layer bug.

### Failure shape

`coordination_initialize_team_internal(...)` returns a structured `InitializeReport`, even when a later initialize step fails after `create_team`. In those failure cases the pipeline also runs cleanup and removes the half-created team directory.

But `coordination_initialize_team_internal(...)` in `src-tauri/src/commands/coordination.rs` previously did this after the report came back:

- if `create_team` succeeded at any point
- run:
  - `sync_team_snapshots_after_change(...)`
  - `sync_active_team_projects_after_change(...)`

That happened **even when**:

- `report.failed_step` was already set
- the initialize pipeline had already cleaned the team back up

So a real later initialize failure could be overwritten by a secondary read:

- `TeamConfigStore::load(... espn_fantasy-team/config.json)`

after cleanup had already deleted the team folder.

That produces the misleading raw IPC error:

- `team config not found ... config.json`

instead of the real failed initialize step.

## Local reproduction of the masking bug

I reproduced the same failure class locally with a regression test:

- `initialize_failure_after_team_creation_does_not_get_rewritten_to_config_missing`

The test simulates:

1. team creation succeeds
2. a later initialize step fails (`send_onboarding`)
3. cleanup removes the team directory
4. the command layer must still return the structured failed-step report instead of rewriting the outcome to `team config not found`

This matches the production symptom pattern for `espn_fantasy-team`.

## Fix landed

Commit:

- `56f1c2b` `Preserve structured team initialize failures`

What changed:

- `src-tauri/src/commands/coordination.rs`
  - post-initialize team sync now runs only when the initialize report actually succeeded
  - it no longer runs just because `create_team` appears in `succeeded_steps`
- `src-tauri/src/commands/coordination/tests.rs`
  - added the regression test that proves failed initialize reports are no longer rewritten to raw missing-config errors after cleanup

## What this means for `espn_fantasy-team`

### Confirmed

- the current production error string is misleading
- it is not enough to conclude that the root cause is a stale team artifact
- it is not enough to conclude that the root cause is a direct UNC-vs-`/home` config lookup mismatch

### Most likely interpretation

`espn_fantasy-team` is failing at a **later initialize step**, after `create_team`, and cleanup is deleting the team before the command layer performs its post-report sync.

Because of the masking bug, the real failed step was lost and replaced with the secondary `config.json not found` error.

## Remaining uncertainty

This investigation fixes the masking bug and rules out the simplest “wrong final config path family” theory for the visible error.

What it does **not** yet reveal from the existing production log is the original failing step for the live `espn_fantasy-team` attempt. That needs one more live repro after the masking fix, so Taurhaus can surface the actual failed step and message instead of the cleanup-aftereffect.

If the next live repro still points to a path issue, the most likely remaining place is not the final team config root but one of the later initialize steps:

- pane/session launch
- team metadata sync
- onboarding delivery
- a project-path consumer that distinguishes Linux project paths from Windows/UNC paths

## Exact verification run

- `cargo test initialize_failure_after_team_creation_does_not_get_rewritten_to_config_missing --manifest-path src-tauri/Cargo.toml`
- `cargo test initialize_error_case_returns_structured_failed_step_report --manifest-path src-tauri/Cargo.toml`

Both passed.
