# Windows freeze after task state update investigation

## Scope

Task `#1279`: investigate the Windows freeze reported shortly after task state changes in a Claude-led project, and treat the fact that Claude mutates task state through its file/update path rather than Mesh CLI as a primary clue.

## Live evidence reviewed

- Windows production log:
  `C:\Users\mstie\AppData\Roaming\com.taurhaus.dev\taurhaus.log.jsonl`
- Resource monitor sample:
  `/tmp/taurhaus-resource-monitor-v2.csv`

## What the live traces showed

The freeze window around `2026-03-13T18:44Z` to `18:46Z` did not show a single backend crash. Instead it showed sustained background churn:

- repeated `coordination.compaction.extractor.heartbeat`
- repeated `backend.session_scanner.scan.completed`
- several slower scanner cycles in the same period
- sustained `taurhaus-daemon` CPU in the resource CSV, generally around `14%` to `23%`

That pattern is consistent with the app staying alive but spending too much time in repeated coordination/runtime refresh work on Windows.

## Claude-specific task mutation path

The affected team/agent being Claude Code matters because Claude does not mutate task state through Mesh CLI task commands. It writes task changes through the Claude task-file lane under `~/.claude/tasks/...`, and Taurhaus ingests those file changes.

Relevant path:

1. watcher classifies Claude task-file changes
2. `event_processor.rs` routes them into `TaskScanTrigger::ClaudeTaskPaths(...)`
3. `bootstrap.rs` runs the debounced task scan loop
4. `services/task_sync.rs::persist_task_scan_with_generation(...)` persists the scan result
5. that path then calls `coordination::operational_context::sync_project_task_snapshots(...)`

So the post-ingest operational snapshot sync is part of the Claude mutation path and runs immediately after the file-driven task-state update.

## Root cause

The expensive part was not the Claude task-file write itself. The real problem was the follow-on operational snapshot refresh path:

- `sync_project_task_snapshots(...)` walked every matching team member for the project
- for each member, `sync_member_snapshot(...)` loaded the project task set again
- it then rebuilt and saved the operational snapshot even when the effective task context had not changed
- the saved snapshot always had a fresh `updated_at`, so identical task state still became a filesystem write

On Windows this is costly because those coordination files live on the Mesh/Claude team-state path, which is typically accessed through WSL/UNC-backed filesystem semantics. After every Claude task-file mutation, Taurhaus could perform redundant no-op operational snapshot rewrites for the same project members. That amplified the post-mutation work and lined up with the sustained backend CPU and apparent UI freeze.

## Fix

Bounded fix landed in:

- `src-tauri/src/coordination/operational_context.rs`
- `src-tauri/src/services/task_sync.rs`

Changes:

- load the project task set once per project snapshot-sync pass instead of once per member
- compute the effective member task snapshot from that shared task set
- skip operational snapshot writes when the snapshot content is unchanged
- preserve the prior `updated_at` when nothing changed, so no-op refreshes do not become writes
- apply the same no-op-save guard to delivery-context persistence too

## Why this addresses the freeze

After a Claude-side task-file mutation, Taurhaus still ingests the real task change, but it no longer turns unchanged member operational state into repeated write traffic. That removes the unnecessary Windows filesystem churn from the exact post-ingest path that was running during the freeze window.

## Regression coverage

Added:

- `coordination::operational_context::tests::sync_member_snapshot_preserves_timestamp_when_task_context_is_unchanged`
- `services::task_sync::tests::persist_task_scan_skips_operational_snapshot_rewrite_when_task_context_is_unchanged`

Existing behavior kept green:

- `services::task_sync::tests::persist_task_scan_updates_operational_snapshot_when_owner_changes`

## Tests run

- `cargo fmt --manifest-path src-tauri/Cargo.toml`
- `cargo test sync_member_snapshot_preserves_timestamp_when_task_context_is_unchanged --manifest-path src-tauri/Cargo.toml`
- `cargo test persist_task_scan_skips_operational_snapshot_rewrite_when_task_context_is_unchanged --manifest-path src-tauri/Cargo.toml`
- `cargo test persist_task_scan_updates_operational_snapshot_when_owner_changes --manifest-path src-tauri/Cargo.toml`

## Remaining risk

The production log does not yet emit a first-class structured event for the Claude task-scan/snapshot-sync leg itself, so the live correlation still depends on code-path analysis plus the surrounding timing/resource evidence. The fix is nevertheless bounded to the Claude post-ingest path that was doing avoidable work, and the new regressions lock that behavior down.
