# IPC command reference

Reference for taurhaus Tauri IPC commands exposed from `src-tauri/src/commands/` and consumed by the frontend.

## Overview

The backend currently registers 65 `#[tauri::command]` functions (with `mesh-bridged-backend` enabled). Command names are snake_case (for example, `get_project`), while frontend wrapper arguments are camelCase (for example, `projectId`) via Tauri's serde argument mapping.

## Projects commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `list_projects` | none | `Result<Vec<ProjectSummary>, String>` | `projects.rs` | Lists all registered projects for the sidebar/project picker. |
| `get_project` | `projectId: string` | `Result<ProjectDetail, String>` | `projects.rs` | Returns full metadata and summary data for one project. |
| `register_project` | `path: string`, `name?: string` | `Result<ProjectDetail, String>` | `projects.rs` | Registers a project path in the database and returns saved details. |
| `update_project` | `projectId: string`, `fields: UpdateProjectFields` | `Result<ProjectDetail, String>` | `projects.rs` | Updates mutable project fields and returns the updated record. |
| `remove_project` | `projectId: string` | `Result<(), String>` | `projects.rs` | Removes a project and related index/session metadata. |
| `is_first_run` | none | `Result<bool, String>` | `projects.rs` | Returns whether the app has zero registered projects. |
| `register_projects_batch` | `paths: string[]` | `Result<Vec<BatchRegistrationResult>, String>` | `projects.rs` | Registers multiple paths and returns per-path success/error results. |
| `scan_directory` | `path: string` | `Result<Vec<DiscoveredProject>, String>` | `projects.rs` | Scans a directory for candidate project roots. |
| `validate_project_path` | `path: string` | `Result<PathValidation, String>` | `projects.rs` | Validates existence/git status/already-registered status for a path. |

## Git commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_recent_commits` | `projectId: string`, `limit?: number` | `Result<Vec<Commit>, String>` | `git.rs` | Returns recent commits for a project. |
| `get_all_commits` | `projectId: string`, `limit?: number`, `offset?: number` | `Result<Vec<Commit>, String>` | `git.rs` | Returns paginated commit history for a project. |
| `get_git_status` | `projectId: string` | `Result<GitStatus, String>` | `git.rs` | Returns branch/dirty/ahead-behind git status for a project. |
| `get_remote_url` | `projectId: string` | `Result<Option<String>, String>` | `git.rs` | Returns normalized remote URL (prefers `origin`; SSH remotes normalized to HTTPS). |
| `get_commit_files` | `projectPath: string`, `hash: string` | `Result<Vec<CommitFile>, String>` | `tasks.rs` | Returns files changed by a specific commit hash. |
| `get_commit_diff` | `projectPath: string`, `hash: string`, `filePath: string` | `Result<Vec<DiffHunk>, String>` | `tasks.rs` | Returns parsed diff hunks for one file in one commit. |
| `get_commits_in_range` | `projectPath: string`, `after: string`, `before: string` | `Result<GitCommitsInRangeResult, String>` | `tasks.rs` | Returns commits and changed files for a time range. |

## Files commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_file_tree` | `projectId: string` | `Result<Vec<FileTreeNode>, String>` | `files.rs` | Returns a hierarchical file tree for the selected project. |
| `read_file` | `projectId: string`, `relativePath: string` | `Result<FileContent, String>` | `files.rs` | Reads text file contents and language metadata. |
| `get_readme` | `projectId: string` | `Result<Option<FileContent>, String>` | `files.rs` | Reads a project README when present. |
| `read_project_asset` | `projectId: string`, `relativePath: string` | `Result<String, String>` | `files.rs` | Reads a binary asset and returns a `data:` URI string. |
| `check_path_type` | `projectId: string`, `relativePath: string` | `Result<String, String>` | `files.rs` | Returns `file`, `directory`, or `not_found` with traversal/symlink-escape protections. |
| `list_directory` | `path: string` | `Result<Vec<DirectoryEntry>, String>` | `projects.rs` | Lists immediate directory entries for the add-project browser. |
| `get_system_roots` | none | `Vec<DirectoryEntry>` | `projects.rs` | Returns root mount points/drives for the directory browser. |

## Search commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `search` | `query: string`, `limit?: number` | `Result<Vec<SearchResult>, String>` | `search.rs` | Runs full-text search over indexed project content. |
| `get_index_status` | none | `Result<IndexStatus, String>` | `search.rs` | Returns index health and document count status. |
| `rebuild_index` | none | `Result<usize, String>` | `search.rs` | Rebuilds the search index from database/project source data. |

## Sessions commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_latest_session` | `projectId: string` | `Result<Option<SessionDetail>, String>` | `sessions.rs` | Returns the latest session for a project, if one exists. |
| `list_sessions` | `projectId: string`, `limit?: number`, `offset?: number` | `Result<Vec<SessionSummary>, String>` | `sessions.rs` | Returns paginated session summaries for a project. |
| `get_session` | `sessionId: string` | `Result<SessionDetail, String>` | `sessions.rs` | Returns full details for a specific session ID. |

## Relationships commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_relationships` | `projectId: string` | `Result<Vec<Relationship>, String>` | `relationships.rs` | Lists non-dismissed project relationships. |
| `dismiss_relationship` | `relationshipId: string` | `Result<(), String>` | `relationships.rs` | Soft-dismisses a relationship from active views. |
| `create_relationship` | `sourceId: string`, `targetId: string`, `relationshipType: string` | `Result<Relationship, String>` | `relationships.rs` | Creates a manual relationship edge between two projects. |
| `remove_relationship` | `relationshipId: string` | `Result<(), String>` | `relationships.rs` | Permanently removes a relationship record. |

## Command center commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `list_claude_sessions` | none | `Result<Vec<ClaudeSession>, String>` | `command_center.rs` | Lists active CLI sessions discovered from tmux/daemon state. |
| `launch_claude_session` | `projectId: string`, `mode: LaunchMode`, `cliTool?: CliTool \| null` | `Result<LaunchSessionResult, String>` | `command_center.rs` | Starts a new Claude/Codex/Gemini session for a project. |
| `stop_claude_session` | `tmuxPane: string`, `cliTool?: CliTool \| null` | `Result<(), String>` | `command_center.rs` | Stops a running session by tmux pane ID. |
| `navigate_to_session` | `tmuxSession: string`, `tmuxWindow: string`, `tmuxPane: string`, `openTerminal?: boolean` | `Result<(), String>` | `command_center.rs` | Focuses/navigates the desktop terminal to a target session pane. |
| `record_session_activity` | `projectPath: string`, `cliTool: string`, `startedAt: string`, `endedAt: string`, `activeDurationMs: number`, `totalDurationMs: number` | `Result<(), String>` | `command_center.rs` | Persists measured activity stats for a completed session. |
| `get_project_activity` | `projectPath: string` | `Result<ProjectActivityStats, String>` | `command_center.rs` | Returns aggregated activity totals for a project path. |

Session update behavior:
- Tauri runtime uses event-driven `sessions-updated` (daemon long-poll bridge) for ongoing updates.
- `list_claude_sessions` is still used for startup snapshot hydrate and for frontend-only mock-mode polling.

## Tasks commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_project_tasks` | `projectPath: string` | `Result<TaskResult, String>` | `tasks.rs` | Returns persisted unified tasks (including `source_key`, archive metadata fields) for the project. |
| `get_task_detail` | `projectPath: string`, `taskId: string`, `source: string`, `sourceKey: string` | `Result<TaskDetail, String>` | `tasks.rs` | Returns enriched task detail resolved by identity tuple `(source, sourceKey, taskId)`. |
| `get_archived_sessions` | `projectPath: string` | `Result<ArchivedSessionsResult, String>` | `tasks.rs` | Returns archived session timeline data for history views. |

## Daemon commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_platform` | none | `String` | `daemon.rs` | Returns normalized host platform (`macos`, `linux`, `windows`). |
| `get_daemon_status` | none | `Result<DaemonStatus, String>` | `daemon.rs` | Returns connection and runtime status for the daemon client. |
| `start_daemon` | none | `Result<String, String>` | `daemon.rs` | Starts/restarts daemon processes and reapplies file watches. |
| `stop_daemon` | none | `Result<String, String>` | `daemon.rs` | Stops daemon process management and watch loops. |
| `check_daemon_install_status` | none | `Result<DaemonInstallStatus, String>` | `daemon.rs` | Returns daemon install/version/update status for onboarding UI. |
| `install_daemon` | none | `Result<String, String>` | `daemon.rs` | Installs bundled daemon binary (or updates existing install). |

## Mesh install commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `check_mesh_install_status` | none | `Result<MeshInstallStatus, String>` | `mesh.rs` | Checks installed mesh version vs bundled version and reports availability (native/WSL-aware). |
| `install_mesh` | none | `Result<String, String>` | `mesh.rs` | Installs bundled mesh binary into the active environment (native or WSL). |

## Settings commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_settings` | none | `Result<Settings, String>` | `settings.rs` | Returns persisted app settings. |
| `update_settings` | `settings: Settings` | `Result<Settings, String>` | `settings.rs` | Replaces persisted settings and returns the saved value. |

## Logging commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `frontend_log` | `level: string`, `message: string` | `()` | `logging.rs` | Writes frontend log events to the shared backend log file. |

## Terminal settings

No standalone terminal-specific Tauri commands are currently registered. Terminal preferences are read/written through `get_settings` and `update_settings`; backend helper functions live in `terminal_settings.rs`.

## Coordination commands (mesh)

These commands are feature-gated behind `mesh-bridged-backend` (enabled by default in this project).

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `coordination_create_team` | `teamName: string` | `Result<(), String>` | `coordination.rs` | Creates a persisted coordination team shell. |
| `coordination_disband_team` | `teamName: string` | `Result<DisbandTeamResponse, String>` | `coordination.rs` | Disbands a team and returns status metadata for UI messaging. |
| `coordination_add_member` | `teamName: string`, `memberName: string`, `backendKind: string` | `Result<(), String>` | `coordination.rs` | Adds a member definition to a team configuration. |
| `coordination_remove_member` | `teamName: string`, `memberName: string` | `Result<RemoveAgentReport, String>` | `coordination.rs` | Removes a member and returns teardown step diagnostics + warnings. |
| `coordination_list_teams` | none | `Result<TeamDiscoveryResponse, String>` | `coordination.rs` | Lists discoverable coordination teams. |
| `coordination_get_team_status` | `teamName: string` | `Result<TeamStatus, String>` | `coordination.rs` | Returns persisted team configuration and health summary. |
| `coordination_initialize_team` | `request: InitializeTeamRequest` | `Result<InitializeReport, String>` | `coordination.rs` | Executes full team bootstrap (tmux, sessions, mesh onboarding). |
| `coordination_add_agent` | `request: AddAgentRequest` | `Result<AddAgentReport, String>` | `coordination.rs` | Hot-adds one agent to an existing coordinated team. |
| `coordination_reonboard` | `request: ReonboardRequest` | `Result<DeliveryResult, String>` | `coordination.rs` | Re-sends onboarding guidance to one member. |
| `coordination_get_live_team_status` | `teamName: string` | `Result<LiveTeamStatus, String>` | `coordination.rs` | Returns runtime/live roster state (session status + pane IDs). |
| `coordination_preflight_check` | `request: InitializeTeamRequest` | `Result<PreflightReport, String>` | `coordination.rs` | Validates prerequisites before initialization. |
| `coordination_get_feature_availability` | none | `Result<FeatureAvailabilityReport, String>` | `coordination.rs` | Reports mesh/tmux feature availability for UI gating. |

## Frontend usage

### Wrapper pattern in `src/lib/ipc.js`

`src/lib/ipc.js` centralizes command calls via:

- `invokeOrMock(command, args, mockFn)`
- If `isTauri()` is true: dynamically imports `@tauri-apps/api/core` and calls `invoke(command, args)`
- If false (frontend-only dev mode): returns `mockFn()` fallback data from `mockData.js`

This keeps UI code stable between full Tauri runs (`just dev`) and frontend-only runs.

### Error handling behavior

`ipc.js` does not swallow backend errors; rejected `invoke(...)` promises propagate to callers.

Exceptions used intentionally in this codebase:

| Location | Command(s) | Behavior |
|---|---|---|
| `src/lib/logger.js` | `frontend_log` | Best-effort logging bridge; errors are ignored with `.catch(() => {})`. |
| `src/App.svelte` | `start_daemon` | Retry action invokes daemon start and ignores errors to keep splash flow non-blocking. |

### Wrapper coverage and direct invokes

Most commands in this reference have first-class wrappers in `ipc.js`. Notable direct/backend-only exceptions:

| Command | `ipc.js` wrapper | Notes |
|---|---|---|
| `frontend_log` | No | Called directly from `src/lib/logger.js` to patch console output. |
| `start_daemon` | No | Called directly from `src/App.svelte` retry handler. |
| `stop_daemon` | No | Registered backend command; currently no direct frontend wrapper call path. |

Related frontend IPC surfaces:

- `onCoordinationStepProgress(callback)` listens to the `coordination-step-progress` event channel (`@tauri-apps/api/event`)
- `openExternalUrl(url)` calls plugin command `plugin:opener|open_url` (not a backend Rust command)

## Key files

| File | Purpose |
|---|---|
| `src-tauri/src/lib.rs` | Registers all Tauri IPC commands via `generate_handler![]`. |
| `src-tauri/src/commands/` | Backend command modules and command function definitions. |
| `src/lib/ipc.js` | Frontend wrappers, invoke/mocks behavior, and command call surface. |
| `src/lib/logger.js` | Direct frontend-to-backend logging IPC bridge for `frontend_log`. |

## Related documents

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — system overview and module structure
- [Mesh view design](../mesh-view-design.md) — UI/behavior context for coordination-related IPC commands
