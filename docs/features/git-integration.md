# Git integration

Git integration gives each project a native commit browser, file-level detail view, and inline patch inspection.

## Overview

taurhaus exposes git history through two surfaces:
- Sidebar/overview status context (`branch`, dirty indicator, activity)
- Full `Git` tab for commit exploration, range filtering, and patch-level inspection

All git operations use Rust (`git2`/libgit2) through backend providers. taurhaus does not shell out to the `git` CLI.

## Commit history

The Git tab shows a paginated commit list with:
- short hash
- subject line
- author
- relative date
- optional grouped date headers (`Today`, `Yesterday`, weekday, calendar date)

Pagination behavior:
- backend supports `limit` + `offset`
- Git tab loads `50` commits per page (`PAGE_SIZE = 50`)
- infinite scrolling loads additional pages when the sentinel enters view

## Commit detail

Selecting a commit opens detail in the left pane:
- commit header: hash, author, relative date
- commit subject + optional body (multi-line messages preserve body)
- changed-file list with file status (`added`, `modified`, `deleted`, `renamed`)

Selecting a file opens its patch view; selecting again collapses back to file list.

## Inline diffs

Per-file patch view provides:
- unified inline hunks (`@@ -old,+new @@`)
- line-level origin markers (`+`, `-`, context)
- old/new line numbers
- color-coded additions and deletions
- quick `Open file` action that jumps to the first added line when available

Current behavior notes:
- Diff presentation is inline (unified), not side-by-side.
- Diff lines are color-coded; language syntax highlighting is not applied in this view.

## Blame view

Current status in this codebase:
- No dedicated blame IPC command or Git tab blame panel is implemented.
- Per-line author/commit blame annotations are therefore not currently exposed in the UI.

## Git status

Project git status includes:
- branch name (or short commit SHA in detached HEAD)
- dirty/clean state (tracked + untracked changes)
- ahead/behind fields in the data model

Current behavior notes:
- `ahead` and `behind` are present in the API model but currently returned as `0` by the local status implementation.
- Branch and dirty state are surfaced in the sidebar and overview header.

## Range filtering

The Git tab supports date-range filtering (`after` / `before` RFC3339 timestamps):
- loads commits within a session/range window
- shows an active filter banner with formatted bounds
- `Clear` resets to standard paginated history

Range filtering is used for session-aware workflows (for example narrowing history to a specific work interval).

## Navigation and per-project memory

When switching projects, taurhaus persists Git tab position per project:
- selected commit hash
- active range filter

On return to a project, Shell restores the Git tab selection/filter so users resume where they left off.

## Implementation model

Git data flow is provider-based:
- frontend calls fine-grained IPC commands (`get_all_commits`, `get_git_status`, `get_commit_files`, `get_commit_diff`, `get_commits_in_range`)
- backend resolves project provider (local filesystem vs daemon bridge for WSL paths)
- provider executes git operations via Rust modules in `src-tauri/src/git/`

Key properties:
- libgit2 in-process operations
- no dependency on external `git` subprocess execution
- same commit/diff primitives reused by both Git tab and session-range enrichment

## Key files

| File | Purpose |
|------|---------|
| `src/lib/GitTab.svelte` | Git tab UI: commit list, commit detail, diff view, range filter, infinite scroll |
| `src/Shell.svelte` | Per-project position memory and restore logic for Git tab state |
| `src-tauri/src/commands/git.rs` | Commit list and git status IPC (`get_recent_commits`, `get_all_commits`, `get_git_status`) |
| `src-tauri/src/commands/tasks.rs` | Commit detail/range IPC used by Git tab (`get_commit_files`, `get_commit_diff`, `get_commits_in_range`) |
| `src-tauri/src/git/commits.rs` | Commit traversal, range filtering, changed files, diff hunks |
| `src-tauri/src/git/status.rs` | Branch + dirty-state status computation |
| `src-tauri/src/provider/mod.rs` | Provider abstraction for git/file operations |
| `src-tauri/src/provider/local.rs` | Local provider implementation using git modules |

## Related documents

- [Project management](./project-management.md) — how sidebar state and project selection interact with git context
- [Platform abstraction](../platform-abstraction.md) — local vs daemon path handling across platforms
