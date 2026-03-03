# Task board

The task board aggregates work items from Claude Code, Codex CLI, and Gemini CLI into one per-project view. It provides a Kanban-style active board, task detail enrichment, and a history view grouped by archived sessions.

## Overview

Task board data is normalized into `UnifiedTask` records in the backend (`task_scanner/types.rs`) and surfaced through task IPC commands in `commands/tasks.rs`.

Frontend layout (`TaskBoard.svelte`) has two sub-tabs:

- `Active`: three status columns (`In Progress`, `Pending`, `Completed`)
- `History`: session-grouped archived work (`SessionHistory.svelte`)

## Task sources

![Task Aggregation Pipeline](../images/task-aggregation.jpg)

Each CLI tool uses a different source format, and the scanner unifies them.

### Claude Code

Source:

- `~/.claude/tasks/{session-id}/*.json` (structured task files)

Parsing (`task_scanner/claude.rs`):

- Uses live `session_id` when available.
- Falls back offline by scanning project slug under `~/.claude/projects/` and finding the newest session with tasks.
- Preserves rich fields: `description`, `activeForm`, `blocks`, `blockedBy`, `owner`, `session_id`.
- Excludes `status: deleted` tasks.

### Codex CLI

Source:

- `update_plan` function-call entries in session JSONL

Parsing (`task_scanner/codex.rs`):

- Uses live `jsonl_path` when available.
- Falls back offline by scanning `~/.codex/sessions/YYYY/MM/DD/` (7-day lookback) and matching `session_meta.payload.cwd` to project path.
- Reads only tail of large JSONL files (256 KB) and parses the last `update_plan` call.
- Maps plan step statuses to unified task status and emits synthetic IDs (`codex-0`, `codex-1`, ...).

### Gemini CLI

Source:

- `TODO.md` in the project root

Parsing (`task_scanner/gemini.rs`):

- Parses markdown checkbox lines (`- [ ]`, `- [x]`, `- [X]`).
- Uses line-number-based IDs (`todo-<line>`).
- Returns empty list when file missing; enforces 1 MB max file size.

## Aggregation and normalization

`task_scanner::get_tasks_for_project` orchestrates all three parsers and returns:

- `tasks: Vec<UnifiedTask>`
- `errors: Vec<(source, message)>`

Behavior:

- One source failing does not block other sources.
- Status values are normalized to `pending`, `in_progress`, `completed`.
- Source is normalized to `claude`, `codex`, or `gemini`.

## Persistence and refresh pipeline

Task board reads persisted tasks from SQLite; scanning is background/event-driven.

Flow:

1. Scanner collects current tasks from source files.
2. `persist_task_scan` upserts into DB (`task_queries::upsert_tasks`).
3. `prune_stale_tasks` archives/deletes tasks missing from current scan per source.
4. Backend emits `project-tasks-changed`.
5. `TaskBoard.svelte` listens and re-fetches via `get_project_tasks(projectPath)`.

`get_project_tasks` itself is a DB read and does not re-scan files in request path.

## Kanban display

`TaskBoard.svelte` groups tasks by status and renders three columns:

- In Progress
- Pending
- Completed

Card content includes:

- Source icon (Claude/Codex/Gemini)
- Subject (+ line-through when completed)
- Optional description preview
- Optional dependency/owner metadata

Current interaction model is click-select rather than drag-and-drop reorder/move; there are no drag handlers in `TaskBoard.svelte`.

## Task detail panel

Selecting a task opens `TaskDetailPanel.svelte` with enriched data from `get_task_detail`:

- Source/tool indicator + status badge
- Full markdown-rendered description
- Session metadata (if task has `session_id`)
- Commits and files changed during inferred session time window
- Dependencies (`blocked_by`, `blocks`) with click-to-navigate chips
- Owner (when present)

Backend enrichment (`commands/tasks.rs`):

- Resolves task from DB by `source + task_id`.
- If `session_id` exists, derives session time range and queries commits/files changed in that window.

## History view

`SessionHistory.svelte` uses `get_archived_sessions(projectPath)`:

- Groups completed work by `session_id` (or `ungrouped`).
- Sorts reverse-chronological by session start.
- Shows task counts, commit/file counts, and contributing source tools.
- Lazily loads commit/file details for expanded sessions via `get_commits_in_range`.

## Per-project scoping

All board queries are scoped by the active project's path:

- `get_project_tasks(projectPath)`
- `get_task_detail(projectPath, taskId, source)`
- `get_archived_sessions(projectPath)`

`Shell.svelte` changes project context first, then loads scoped task data for that project.

## Position memory (`$bindable` pattern)

Task board state persists across project switches through bound position props.

Mechanism:

- `TaskBoard` exports `position = $bindable(null)` and writes active sub-tab + selected task ID + selected task source.
- `Shell.svelte` stores this in per-project memory (`projectPositions`).
- On project reselect, Shell passes restore target via `navTarget`.
- `TaskBoard` applies restore once tasks finish loading (`pendingRestore`).

Result: users return to the same sub-tab/task context when moving between projects.

## Key files

| File | Purpose |
|---|---|
| `src/lib/TaskBoard.svelte` | Main task board UI, active/history sub-tabs, status columns, selection, restore handling. |
| `src/lib/TaskDetailPanel.svelte` | Task detail side panel with metadata, dependencies, and enriched context sections. |
| `src/lib/SessionHistory.svelte` | Archived-session accordion and lazy commit/file drill-down. |
| `src/lib/taskHelpers.js` | Shared task status labels/badge style mapping. |
| `src-tauri/src/commands/tasks.rs` | Task IPC commands, detail/session enrichment, persistence helpers, scanner integration. |
| `src-tauri/src/task_scanner/mod.rs` | Aggregates Claude/Codex/Gemini scanners with partial-failure handling. |
| `src-tauri/src/task_scanner/claude.rs` | Claude structured task JSON parser + offline fallback lookup. |
| `src-tauri/src/task_scanner/codex.rs` | Codex `update_plan` JSONL parser + offline session matching. |
| `src-tauri/src/task_scanner/gemini.rs` | Gemini `TODO.md` checkbox parser. |
| `src-tauri/src/task_scanner/types.rs` | Unified task DTOs and task board response types. |

## Related documents

- [IPC command reference](../architecture/ipc-reference.md) — task command signatures and return types
- [Session management](./session-management.md) — session detection that feeds task scanner
