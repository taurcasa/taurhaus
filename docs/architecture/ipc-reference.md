# IPC command reference

Reference for taurhaus Tauri IPC commands exposed from `src-tauri/src/commands/` and consumed by the frontend.

## Overview

The backend currently registers a large set of `#[tauri::command]` functions (with `mesh-bridged-backend` enabled). Command names are snake_case (for example, `get_project`), while frontend wrapper arguments are camelCase (for example, `projectId`) via Tauri's serde argument mapping.

## Projects commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `list_projects` | none | `Result<Vec<ProjectSummary>, String>` | `projects.rs` | Lists all registered projects for the sidebar/project picker. |
| `get_project` | `projectId: string` | `Result<ProjectDetail, String>` | `projects.rs` | Returns full metadata and summary data for one project. |
| `register_project` | `path: string`, `name?: string` | `Result<ProjectDetail, String>` | `projects.rs` | Registers a project path in the database and returns saved details. |
| `create_project` | `name: string`, `parentDir: string` | `Result<ProjectDetail, String>` | `projects.rs` | Creates a new git-initialized directory and registers it as a project. |
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
| `list_cli_sessions` | none | `Result<Vec<ClaudeSession>, String>` | `command_center.rs` | Lists active CLI sessions discovered from tmux/daemon state. |
| `launch_cli_session` | `projectId: string`, `mode: LaunchMode`, `cliTool?: CliTool \| null` | `Result<LaunchSessionResult, String>` | `command_center.rs` | Starts a new Claude/Codex/Gemini session for a project. |
| `stop_cli_session` | `tmuxPane: string`, `cliTool?: CliTool \| null` | `Result<(), String>` | `command_center.rs` | Stops a running session by tmux pane ID. |
| `navigate_to_session` | `tmuxSession: string`, `tmuxWindow: string`, `tmuxPane: string`, `openTerminal?: boolean` | `Result<(), String>` | `command_center.rs` | Focuses/navigates the desktop terminal to a target session pane. |
| `record_session_activity` | `projectPath: string`, `cliTool: string`, `startedAt: string`, `endedAt: string`, `activeDurationMs: number`, `totalDurationMs: number` | `Result<(), String>` | `command_center.rs` | Persists measured activity stats for a completed session. |
| `get_project_activity` | `projectPath: string` | `Result<ProjectActivityStats, String>` | `command_center.rs` | Returns aggregated activity totals for a project path. |
| `get_foreground_project` | none | `Result<Option<string>, String>` | `command_center.rs` | Returns the project currently owning foreground tmux focus, when known. |

Session update behavior:
- Tauri runtime uses event-driven `sessions-updated` (daemon long-poll bridge) for ongoing updates.
- `list_cli_sessions` is still used for startup snapshot hydrate and for frontend-only mock-mode polling.

## Tasks commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_project_tasks` | `projectId: string` | `Result<TaskResult, String>` | `tasks.rs` | Returns persisted unified tasks (including `source_key`, archive metadata fields) for the project. |
| `get_task_detail` | `projectId: string`, `taskId: string`, `source: string`, `sourceKey: string` | `Result<TaskDetail, String>` | `tasks.rs` | Returns enriched task detail resolved by identity tuple `(source, sourceKey, taskId)`. |
| `get_archived_sessions` | `projectId: string` | `Result<ArchivedSessionsResult, String>` | `tasks.rs` | Returns archived session timeline data for history views. |
| `get_commit_files` | `projectId: string`, `hash: string` | `Result<Vec<CommitFile>, String>` | `tasks.rs` | Returns files changed by a specific commit hash. |
| `get_commit_diff` | `projectId: string`, `hash: string`, `filePath: string` | `Result<Vec<DiffHunk>, String>` | `tasks.rs` | Returns parsed diff hunks for one file in one commit. |
| `get_commits_in_range` | `projectId: string`, `after: string`, `before: string` | `Result<GitCommitsInRangeResult, String>` | `tasks.rs` | Returns commits and changed files for a time range. |

## Templates commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `templates_list_roles_full` | none | `Result<Vec<RoleTemplateFull>, String>` | `templates.rs` | Lists role templates with source/read-only metadata. |
| `templates_get_role` | `roleId: string` | `Result<RoleTemplate, String>` | `templates.rs` | Returns one role template by id. |
| `templates_upsert_role` | `request: TemplatesUpsertRoleRequest` | `Result<RoleTemplate, String>` | `templates.rs` | Creates or updates a role template. |
| `templates_delete_role` | `roleId: string` | `Result<(), String>` | `templates.rs` | Deletes a user-defined role template. |
| `templates_list_presets_full` | none | `Result<Vec<TeamPresetFull>, String>` | `templates.rs` | Lists team presets with source/read-only metadata. |
| `templates_get_preset` | `presetId: string` | `Result<TeamPreset, String>` | `templates.rs` | Returns one team preset by id. |
| `templates_upsert_preset` | `request: TemplatesUpsertPresetRequest` | `Result<TeamPreset, String>` | `templates.rs` | Creates or updates a team preset. |
| `templates_delete_preset` | `presetId: string` | `Result<(), String>` | `templates.rs` | Deletes a user-defined team preset. |
| `templates_compose_team` | `request: TemplatesComposeTeamRequest` | `Result<CompositionResult, String>` | `templates.rs` | Composes a runtime roster from role templates and preset-like slot input. |
| `templates_get_storage_status` | none | `Result<TemplateStorageStatus, String>` | `templates.rs` | Returns storage mode, dirty state, and pending actions. |
| `templates_get_history` | `limit?: number`, `cursor?: string \| null` | `Result<TemplateCommitPage, String>` | `templates.rs` | Returns paginated template git history. |
| `templates_get_diff` | `commitId: string` | `Result<TemplateDiff, String>` | `templates.rs` | Returns the template diff for one commit id. |
| `templates_revert` | `request: TemplateRevertRequest` | `Result<(), String>` | `templates.rs` | Reverts a template to a selected commit state. |
| `templates_flush_pending` | none | `Result<TemplateFlushResult, String>` | `templates.rs` | Flushes pending template actions into a commit (used by E2E and maintenance flows). |
| `export_role_to_file` | `request: RoleExportRequest` | `Result<RoleExportResult, String>` | `templates.rs` | Exports a stored role to Claude Code or Copilot custom-agent markdown. |

## Daemon commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_platform` | none | `String` | `daemon.rs` | Returns normalized host platform (`macos`, `linux`, `windows`). |
| `get_daemon_status` | none | `Result<DaemonStatus, String>` | `daemon.rs` | Returns connection and runtime status for the daemon client. |
| `start_daemon` | none | `Result<OperationResult, String>` | `daemon.rs` | Starts/restarts daemon processes and reapplies file watches. |
| `check_daemon_install_status` | none | `Result<DaemonInstallStatus, String>` | `daemon.rs` | Returns daemon install/version/update status for onboarding UI. |
| `install_daemon` | none | `Result<OperationResult, String>` | `daemon.rs` | Installs bundled daemon binary (or updates existing install). |

## Mesh install commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `check_mesh_install_status` | none | `Result<MeshInstallStatus, String>` | `mesh.rs` | Checks installed mesh version vs bundled version and reports availability (native/WSL-aware). |
| `install_mesh` | none | `Result<OperationResult, String>` | `mesh.rs` | Installs bundled mesh binary into the active environment (native or WSL). |

## Settings commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `get_settings` | none | `Result<Settings, String>` | `settings.rs` | Returns persisted app settings. |
| `update_settings` | `settings: Settings` | `Result<Settings, String>` | `settings.rs` | Replaces persisted settings and returns the saved value. |

## Logging commands

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `frontend_log` | `payload?: FrontendLogPayload`, `level?: string` (legacy), `message?: string` (legacy) | `Result<(), String>` | `logging.rs` | Writes frontend structured log events to the shared backend JSONL sink (legacy level/message fields are still accepted). |

## Terminal settings

No standalone terminal-specific Tauri commands are currently registered. Terminal preferences are read/written through `get_settings` and `update_settings`; backend helper functions live in `terminal_settings.rs`.

## Coordination commands (mesh)

These commands are feature-gated behind `mesh-bridged-backend` (enabled by default in this project).

| Command | Parameters (frontend args) | Return type | Module | Description |
|---|---|---|---|---|
| `coordination_create_team` | `teamName: string` | `Result<(), String>` | `coordination.rs` | Creates a persisted coordination team shell. |
| `coordination_disband_team` | `teamName: string` | `Result<DisbandTeamResponse, String>` | `coordination.rs` | Disbands a team and returns status metadata for UI messaging. |
| `coordination_add_member` | `teamName: string`, `memberName: string`, `backendKind: string`, `projectPath?: string \| null` | `Result<(), String>` | `coordination.rs` | Adds a member definition to a team configuration. |
| `coordination_remove_member` | `teamName: string`, `memberName: string` | `Result<RemoveAgentReport, String>` | `coordination.rs` | Removes a member and returns teardown step diagnostics + warnings. |
| `coordination_list_teams` | none | `Result<TeamDiscoveryResponse, String>` | `coordination.rs` | Lists discoverable coordination teams. |
| `coordination_get_team_status` | `teamName: string` | `Result<TeamStatus, String>` | `coordination.rs` | Returns persisted team configuration and health summary. |
| `coordination_initialize_team` | `request: InitializeTeamRequest` | `Result<InitializeReport, String>` | `coordination.rs` | Executes full team bootstrap (tmux, sessions, mesh onboarding). |
| `coordination_add_agent` | `request: AddAgentRequest` | `Result<AddAgentReport, String>` | `coordination.rs` | Hot-adds one agent to an existing coordinated team. |
| `coordination_resume_member` | `request: ResumeMemberRequest` | `Result<ResumeAgentReport, String>` | `coordination.rs` | Resumes an offline member by restoring pane/runtime identity and launching a fresh tool session. |
| `coordination_resume_team` | `request: ResumeTeamRequest` | `Result<ResumeTeamReport, String>` | `coordination.rs` | Resumes the lead first, then same-project and cross-project members, with partial-success reporting. |
| `coordination_reonboard` | `request: ReonboardRequest` | `Result<DeliveryResult, String>` | `coordination.rs` | Re-sends onboarding guidance to one member. |
| `coordination_get_live_team_status` | `teamName: string` | `Result<LiveTeamStatus, String>` | `coordination.rs` | Returns runtime/live roster state (session status + pane IDs). |
| `coordination_get_compaction_audit` | `teamName: string` | `Result<Vec<CompactionAuditEntry>, String>` | `coordination.rs` | Returns current-run compaction audit entries for the Mesh runtime panel. |
| `coordination_preflight_check` | `request: InitializeTeamRequest` | `Result<PreflightReport, String>` | `coordination.rs` | Validates prerequisites before initialization. |
| `coordination_get_feature_availability` | none | `Result<FeatureAvailabilityReport, String>` | `coordination.rs` | Reports mesh/tmux feature availability for UI gating. |
| `coordination_get_project_mesh_snapshot` | `projectId: string` | `Result<ProjectMeshSnapshot, String>` | `coordination.rs` | Returns the project-scoped mesh snapshot used for runtime canvas hydration. |

## Frontend usage

### Wrapper pattern in `src/lib/ipc/`

The frontend IPC layer is split by domain modules under `src/lib/ipc/` and re-exported through `src/lib/ipc.js`.

Shared invoke behavior lives in `src/lib/ipc/client.js`:

- `invokeOrMock(command, args, mockFn)`
- If `isTauri()` is true: dynamically imports `@tauri-apps/api/core` and calls `invoke(command, args)`
- If false (frontend-only dev mode): returns `mockFn()` fallback data from `src/lib/ipc/mocks/`

This keeps UI code stable between full Tauri runs (`just dev`) and frontend-only runs.

### Error handling behavior

Domain wrappers in `src/lib/ipc/*.js` do not swallow backend errors; rejected `invoke(...)` promises propagate to callers.

Exceptions used intentionally in this codebase:

| Location | Command(s) | Behavior |
|---|---|---|
| `src/lib/logger.js` | `frontend_log` | Best-effort logging bridge; errors are ignored with `.catch(() => {})`. |
| `src/App.svelte` | `start_daemon` | Retry action invokes daemon start and ignores errors to keep splash flow non-blocking. |

### Wrapper coverage and direct invokes

Most commands in this reference have first-class wrappers in `src/lib/ipc/*.js`. Notable direct/backend-only exceptions:

| Command | `ipc.js` wrapper | Notes |
|---|---|---|
| `frontend_log` | No | Called directly from `src/lib/logger.js` to patch console output. |

Related frontend IPC surfaces:

- `onCoordinationStepProgress(callback)` listens to the `coordination-step-progress` event channel (`@tauri-apps/api/event`)
- `openExternalUrl(url)` calls plugin command `plugin:opener|open_url` (not a backend Rust command)

## Key files

| File | Purpose |
|---|---|
| `src-tauri/src/lib.rs` | Registers all Tauri IPC commands via `generate_handler![]`. |
| `src-tauri/src/commands/` | Backend command modules and command function definitions. |
| `src/lib/ipc/` | Frontend IPC domain modules (`client`, `projects`, `sessions`, `tasks`, `templates`, `coordination`, `system`) and mocks. |
| `src/lib/ipc.js` | Thin compatibility re-export for the `src/lib/ipc/` module surface. |
| `src/lib/logger.js` | Direct frontend-to-backend logging IPC bridge for `frontend_log`. |

## Related documents

- [ARCHITECTURE.md](../../ARCHITECTURE.md) — system overview and module structure
- [Mesh feature guide](../features/mesh.md) — user-facing coordination flows that map onto these IPC commands
- [Coordination architecture](../coordination-architecture.md) — backend structure behind the coordination command set
