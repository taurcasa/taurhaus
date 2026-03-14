# Remove Foreground Selection Contention And Reconcile

**Date:** 2026-03-14
**Task:** #1297

## Summary

Removed two distinct sources of foreground project-switch cost:

1. `get_project(...)` no longer performs project-activity promotion or watcher
   reconcile work as part of the foreground selection read.
2. WSL daemon-backed foreground selection reads now fail fast when the shared
   daemon lane is already busy, instead of blocking behind restore-time daemon
   work.

This keeps explicit project switching on the user-facing read path and moves it
away from both immediate maintenance work and shared-daemon lock contention.

## Traced Foreground Path

The Shell selection path still loads six sections in parallel via
`loadProjectSelectionData(...)`:

- project details
- recent commits
- latest session
- session history
- README
- relationships

Two backend behaviors were causing selection to inherit extra cost:

### 1. `get_project(...)` was not a pure read

Before this change, `src-tauri/src/commands/projects.rs` did all of the
following inside the foreground detail read:

- `project::touch_activity(...)`
- `project::get_project(...)`
- `enqueue_activity_watch_reconcile(..., "project_selected")`

So a user click was not just reading the selected project. It also immediately
triggered maintenance work:

- activity promotion write
- activity-watch reconcile
- local/daemon watcher evaluation

### 2. Daemon-backed selection reads still serialized on the shared daemon lane

The provider-side shared daemon connection still uses a single mutex-backed TCP
stream for normal project reads. Only `send_status_request(...)` had been
hardened to fast-fail on a busy connection. Ordinary selection reads still went
through the blocking shared lane.

For WSL-backed project selection, the most important affected foreground reads
were:

- `get_recent_commits(...)`
- `get_readme(...)`

That meant restore-time daemon work could directly stall the next explicit
project switch.

## Changes Implemented

### 1. Made `get_project(...)` a true foreground read again

`src-tauri/src/commands/projects.rs` now routes project selection detail through
an explicit read-only helper:

- `get_project_detail_for_selection(...)`

The command no longer:

- touches project activity on read
- enqueues watcher reconcile on `project_selected`

This removes immediate maintenance cost from the foreground project-detail path.

### 2. Added fast-fail guards for daemon-backed foreground selection reads

`src-tauri/src/commands/git.rs` and `src-tauri/src/commands/files.rs` now
check for:

- WSL-backed project path
- connected daemon
- daemon shared lane currently busy

If all three are true, the command returns a sanitized daemon transport error
immediately instead of waiting behind the shared daemon connection lock.

Covered commands:

- `get_recent_commits(...)`
- `get_readme(...)`

That preserves the existing degraded-loading behavior already present in the
frontend selection flow while removing the direct foreground stall.

## Evidence

Regression coverage added:

- `commands::projects::tests::selection_detail_read_does_not_promote_activity_or_queue_maintenance`
- `commands::git::tests::recent_commits_foreground_read_fails_fast_when_daemon_lane_is_busy`
- `commands::files::tests::readme_foreground_read_fails_fast_when_daemon_lane_is_busy`

Verification run:

- `cd src-tauri && cargo fmt`
- `cd src-tauri && cargo test --lib selection_detail_read_does_not_promote_activity_or_queue_maintenance -- --test-threads=1`
- `cd src-tauri && cargo test --lib daemon_lane_is_busy -- --test-threads=1`

All passed.

## Practical Effect

Foreground project switching no longer directly pays for:

- `project_selected` watcher reconcile
- selection-triggered activity promotion write
- restore-time shared-daemon contention for recent-commits and README reads

Instead:

- project detail remains a pure DB read
- busy daemon-backed selection sections degrade immediately through the existing
  fallback path instead of blocking the whole selection

This narrows the foreground selection path to user-visible reads and prevents
restore-time background maintenance from being charged directly to the next
explicit project switch.
