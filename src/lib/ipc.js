/**
 * IPC wrapper — thin layer over Tauri invoke() with mock fallback.
 *
 * When running inside Tauri (`just dev`), calls the Rust backend.
 * When running in Vite-only mode (`just dev-frontend`), returns mock data
 * so frontend development works without the Rust backend.
 *
 * Mock data lives in mockData.js — separated to keep command definitions
 * easy to scan and edit independently.
 */
import {
  MOCK_PROJECTS, MOCK_COMMITS, MOCK_DIFF_HUNKS, MOCK_FILE_TREE,
  MOCK_SESSION, MOCK_SESSIONS, MOCK_DETAIL,
  MOCK_SEARCH_RESULTS, MOCK_RELATIONSHIPS, MOCK_SETTINGS,
  MOCK_CLAUDE_SESSIONS,
} from './mockData.js'
import { listen } from '@tauri-apps/api/event'

/** Check whether we're running inside a Tauri webview. */
export function isTauri() {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

async function invokeOrMock(command, args, mockFn) {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core')
    return args !== undefined ? invoke(command, args) : invoke(command)
  }
  return mockFn()
}

// ---------------------------------------------------------------------------
// IPC functions
// ---------------------------------------------------------------------------

/** List all registered projects (sidebar). */
export function listProjects() {
  return invokeOrMock('list_projects', undefined, () => MOCK_PROJECTS)
}

/** Get full project detail by ID. */
export function getProject(projectId) {
  return invokeOrMock('get_project', { projectId }, () => ({
    ...MOCK_DETAIL,
    id: projectId,
  }))
}

/** Register a new project from a filesystem path. */
export function registerProject(path, name) {
  return invokeOrMock('register_project', { path, name }, () => ({
    ...MOCK_DETAIL,
    path,
    name: name || path.split('/').pop(),
  }))
}

/** Update a project's mutable fields. */
export function updateProject(projectId, fields) {
  return invokeOrMock('update_project', { projectId, fields }, () => ({
    ...MOCK_DETAIL,
    id: projectId,
    ...fields,
  }))
}

/** Remove a project by ID. */
export function removeProject(projectId) {
  return invokeOrMock('remove_project', { projectId }, () => undefined)
}

/** Scan a directory for potential projects. */
export function scanDirectory(path) {
  return invokeOrMock('scan_directory', { path }, () => [])
}

/** List subdirectories at a path (for directory tree browser). */
export function listDirectory(path) {
  return invokeOrMock('list_directory', { path }, () => [
    { name: 'project-a', path: `${path}/project-a`, isExpandable: true },
    { name: 'project-b', path: `${path}/project-b`, isExpandable: false },
  ])
}

/** Get filesystem roots (drive letters on Windows, ["/"] on Linux). */
export function getSystemRoots() {
  return invokeOrMock('get_system_roots', undefined, () => [
    { name: '/', path: '/', isExpandable: true },
  ])
}

/** Validate a path: exists, is git repo, is already registered. */
export function validateProjectPath(path) {
  return invokeOrMock('validate_project_path', { path }, () => ({
    exists: true,
    isGitRepo: true,
    isRegistered: false,
  }))
}

// ---------------------------------------------------------------------------
// Git IPC functions
// ---------------------------------------------------------------------------

/** Get recent commits for a project. */
export function getRecentCommits(projectId, limit = 10) {
  return invokeOrMock('get_recent_commits', { projectId, limit }, () => MOCK_COMMITS)
}

/** Get all commits with pagination. */
export function getAllCommits(projectId, limit = 50, offset = 0) {
  return invokeOrMock('get_all_commits', { projectId, limit, offset }, () => MOCK_COMMITS)
}

/** Get git status for a project. */
export function getGitStatus(projectId) {
  return invokeOrMock('get_git_status', { projectId }, () => ({
    branch: 'main',
    is_dirty: false,
    ahead: 0,
    behind: 0,
  }))
}

/** Get normalized remote URL for a project, if available. */
export function getRemoteUrl(projectId) {
  return invokeOrMock('get_remote_url', { projectId }, () => null)
}

// ---------------------------------------------------------------------------
// File IPC functions
// ---------------------------------------------------------------------------

/** Get file tree for a project. */
export function getFileTree(projectId) {
  return invokeOrMock('get_file_tree', { projectId }, () => MOCK_FILE_TREE)
}

/** Read a file's content. */
export function readFile(projectId, relativePath) {
  return invokeOrMock('read_file', { projectId, relativePath }, () => ({
    path: relativePath,
    content: '// Mock file content',
    language: 'javascript',
  }))
}

/** Read a binary asset from a project directory as a base64 data URI. */
export function readProjectAsset(projectId, relativePath) {
  return invokeOrMock('read_project_asset', { projectId, relativePath }, () => null)
}

/** Check if a project-relative path points to a file, directory, or missing entry. */
export function checkPathType(projectId, relativePath) {
  return invokeOrMock('check_path_type', { projectId, relativePath }, () => 'not_found')
}

/** Get the README for a project. */
export function getReadme(projectId) {
  return invokeOrMock('get_readme', { projectId }, () => ({
    path: 'README.md',
    content: '# Mock Project\n\nThis is a mock README.',
    language: 'markdown',
  }))
}

// ---------------------------------------------------------------------------
// Session IPC functions
// ---------------------------------------------------------------------------

/** Get the latest session for a project. */
export function getLatestSession(projectId) {
  return invokeOrMock('get_latest_session', { projectId }, () => MOCK_SESSION)
}

/** List sessions for a project with pagination. */
export function listSessions(projectId, limit = 20, offset = 0) {
  return invokeOrMock('list_sessions', { projectId, limit, offset }, () => MOCK_SESSIONS)
}

/** Get full session detail by ID. */
export function getSession(sessionId) {
  return invokeOrMock('get_session', { sessionId }, () => MOCK_SESSION)
}

// ---------------------------------------------------------------------------
// Search IPC functions
// ---------------------------------------------------------------------------

/** Search across all indexed content. */
export function search(query, limit = 20) {
  return invokeOrMock('search', { query, limit }, () => {
    if (!query || !query.trim()) return []
    return MOCK_SEARCH_RESULTS.filter(r =>
      r.title.toLowerCase().includes(query.toLowerCase()) ||
      r.snippet.toLowerCase().includes(query.toLowerCase())
    )
  })
}

/** Get search index status. */
export function getIndexStatus() {
  return invokeOrMock('get_index_status', undefined, () => ({
    doc_count: 42,
    is_empty: false,
  }))
}

/** Rebuild the search index from scratch. */
export function rebuildIndex() {
  return invokeOrMock('rebuild_index', undefined, () => 42)
}

// ---------------------------------------------------------------------------
// Relationship IPC functions
// ---------------------------------------------------------------------------

/** Get relationships for a project (non-dismissed). */
export function getRelationships(projectId) {
  return invokeOrMock('get_relationships', { projectId }, () => MOCK_RELATIONSHIPS)
}

/** Dismiss a relationship (soft delete). */
export function dismissRelationship(relationshipId) {
  return invokeOrMock('dismiss_relationship', { relationshipId }, () => undefined)
}

/** Create a manual relationship. */
export function createRelationship(sourceId, targetId, relationshipType) {
  return invokeOrMock('create_relationship', { sourceId, targetId, relationshipType }, () => ({
    id: 'rel-new',
    source_project_id: sourceId,
    target_project_id: targetId,
    relationship_type: relationshipType,
    detection_source: 'manual',
    dismissed: false,
    first_detected_at: new Date().toISOString(),
    last_seen_at: new Date().toISOString(),
  }))
}

/** Remove a relationship permanently. */
export function removeRelationship(relationshipId) {
  return invokeOrMock('remove_relationship', { relationshipId }, () => undefined)
}

// ---------------------------------------------------------------------------
// Settings IPC functions
// ---------------------------------------------------------------------------

/** Get current application settings. */
export function getSettings() {
  return invokeOrMock('get_settings', undefined, () => MOCK_SETTINGS)
}

/** Update settings (full replacement). */
export function updateSettings(settings) {
  return invokeOrMock('update_settings', { settings }, () => ({
    ...MOCK_SETTINGS,
    ...settings,
  }))
}

/** Open a URL in the system default browser via the opener plugin. */
export function openExternalUrl(url) {
  return invokeOrMock('plugin:opener|open_url', { url }, () => {
    window.open(url, '_blank')
  })
}

/** Check if this is the first run (no projects registered). */
export function isFirstRun() {
  return invokeOrMock('is_first_run', undefined, () => MOCK_PROJECTS.length === 0)
}

// ---------------------------------------------------------------------------
// Batch Registration IPC functions
// ---------------------------------------------------------------------------

/** Register multiple projects at once. Returns array of results with success/error per path. */
export function registerProjectsBatch(paths) {
  return invokeOrMock('register_projects_batch', { paths }, () =>
    paths.map((path, index) => ({
      path,
      success: true,
      project: {
        ...MOCK_DETAIL,
        id: `mock-batch-${index}`,
        path,
        name: path.split('/').pop(),
      },
      error: null,
    }))
  )
}

// ---------------------------------------------------------------------------
// Command Center — Claude Code session management
// ---------------------------------------------------------------------------

/** List all running Claude Code sessions. */
export function listClaudeSessions() {
  return invokeOrMock('list_claude_sessions', undefined, () => MOCK_CLAUDE_SESSIONS)
}

/** Launch a new CLI tool session for a project. Mode: "continue" | "fresh" | "resume". cliTool: "claude" | "codex" | "gemini" (optional, defaults to "claude"). */
export function launchClaudeSession(projectId, mode, cliTool) {
  return invokeOrMock('launch_claude_session', { projectId, mode, cliTool: cliTool ?? null }, () => ({
    tmux_session: 'taurhaus',
    tmux_window: 'project',
    tmux_pane: '%99',
  }))
}

/** Stop a running CLI tool session by tmux pane ID. cliTool: "claude" | "codex" | "gemini" (optional, defaults to "claude"). */
export function stopClaudeSession(tmuxPane, cliTool) {
  return invokeOrMock('stop_claude_session', { tmuxPane, cliTool: cliTool ?? null }, () => undefined)
}

/** Navigate to a Claude Code session's tmux pane. openTerminal: if true, opens Windows Terminal when not running. */
export function navigateToSession(tmuxSession, tmuxWindow, tmuxPane, openTerminal = false) {
  return invokeOrMock('navigate_to_session', { tmuxSession, tmuxWindow, tmuxPane, openTerminal }, () => undefined)
}

/** Record a completed CLI session's activity stats. */
export function recordSessionActivity(projectPath, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs) {
  return invokeOrMock('record_session_activity', { projectPath, cliTool, startedAt, endedAt, activeDurationMs, totalDurationMs }, () => undefined)
}

/** Get tasks from all CLI tools for a project. */
export function getProjectTasks(projectPath) {
  return invokeOrMock('get_project_tasks', { projectPath }, () => ({
    tasks: [
      { id: '1', source_key: 'sess-aaa-111', subject: 'Add task scanner backend', description: 'Parse tasks from all three CLI tools', active_form: 'Adding task scanner', status: 'in_progress', source: 'claude', blocks: ['2'], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 300000).toISOString(), updated_at: new Date(Date.now() - 300000).toISOString() },
      { id: '2', source_key: 'sess-aaa-111', subject: 'Build TaskBoard UI component', description: null, active_form: null, status: 'pending', source: 'claude', blocks: [], blocked_by: ['1'], owner: null, state_changed_at: new Date(Date.now() - 3600000).toISOString(), updated_at: new Date(Date.now() - 3600000).toISOString() },
      { id: '3', source_key: 'sess-aaa-111', subject: 'Write integration tests', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 7200000).toISOString(), updated_at: new Date(Date.now() - 7200000).toISOString() },
      { id: 'codex-0', source_key: 'legacy-codex', subject: 'Initialize project structure', description: null, active_form: null, status: 'completed', source: 'codex', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 10800000).toISOString(), updated_at: new Date(Date.now() - 10800000).toISOString() },
      { id: 'codex-1', source_key: 'legacy-codex', subject: 'Implement CLI parsing', description: null, active_form: 'Implementing CLI parsing', status: 'in_progress', source: 'codex', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 900000).toISOString(), updated_at: new Date(Date.now() - 900000).toISOString() },
      { id: 'codex-2', source_key: 'legacy-codex', subject: 'Add error handling', description: null, active_form: null, status: 'pending', source: 'codex', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 5400000).toISOString(), updated_at: new Date(Date.now() - 5400000).toISOString() },
      { id: 'todo-1', source_key: 'gemini-todo', subject: 'Write unit tests', description: null, active_form: null, status: 'pending', source: 'gemini', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 1800000).toISOString(), updated_at: new Date(Date.now() - 1800000).toISOString() },
      { id: 'todo-2', source_key: 'gemini-todo', subject: 'Update documentation', description: null, active_form: null, status: 'completed', source: 'gemini', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 14400000).toISOString(), updated_at: new Date(Date.now() - 14400000).toISOString() },
    ],
    errors: [],
  }))
}

/** Get enriched detail for a single task: full data + session info + commits + files changed. */
export function getTaskDetail(projectPath, taskId, source, sourceKey) {
  return invokeOrMock('get_task_detail', { projectPath, taskId, source, sourceKey }, () => ({
    task: {
      id: taskId,
      source_key: sourceKey || (source === 'gemini' ? 'gemini-todo' : source === 'codex' ? 'legacy-codex' : 'legacy-claude'),
      subject: 'Add task scanner backend',
      description: 'Parse tasks from all three CLI tools and present them in a unified task board.',
      active_form: 'Adding task scanner',
      status: 'in_progress',
      source: source || 'claude',
      blocks: ['2'],
      blocked_by: [],
      owner: null,
      session_id: 'abc-123-def',
      state_changed_at: new Date(Date.now() - 3600000).toISOString(),
      updated_at: new Date().toISOString(),
      archived_at: null,
      last_status: 'in_progress',
      archived_reason: null,
    },
    session: {
      id: 'abc-123-def',
      started_at: new Date(Date.now() - 3600000).toISOString(),
      ended_at: new Date().toISOString(),
    },
    commits: [
      { hash: 'abc12345', message: 'Add task scanner types and module scaffold', author: 'Developer', date: '30m' },
      { hash: 'def67890', message: 'Implement Claude task file parser', author: 'Developer', date: '1h' },
      { hash: 'ghi11111', message: 'Add Codex JSONL plan parser', author: 'Developer', date: '2h' },
    ],
    files_changed: [
      'src-tauri/src/task_scanner/mod.rs',
      'src-tauri/src/task_scanner/types.rs',
      'src-tauri/src/task_scanner/claude.rs',
      'src-tauri/src/task_scanner/codex.rs',
      'src-tauri/src/task_scanner/gemini.rs',
    ],
  }))
}

/** Get archived sessions for the session history timeline. */
export function getArchivedSessions(projectPath) {
  return invokeOrMock('get_archived_sessions', { projectPath }, () => ({
    sessions: [
      {
        session_id: 'sess-aaa-111',
        started_at: new Date(Date.now() - 2 * 86400000).toISOString(),
        ended_at: new Date(Date.now() - 2 * 86400000 + 8100000).toISOString(),
        duration_ms: 8100000,
        tasks: [
          { id: '10', source_key: 'sess-aaa-111', subject: 'Add task scanner backend', description: 'Parse tasks from all three CLI tools', active_form: null, status: 'completed', source: 'claude', blocks: ['11'], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 3600000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '11', source_key: 'sess-aaa-111', subject: 'Build TaskBoard UI component', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: ['10'], owner: null, archived_at: new Date(Date.now() - 3500000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '12', source_key: 'sess-aaa-111', subject: 'Write integration tests', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 3400000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: 'todo-5', source_key: 'sess-aaa-111', subject: 'Update README with task board docs', description: null, active_form: null, status: 'completed', source: 'gemini', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 3300000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: 'codex-3', source_key: 'sess-aaa-111', subject: 'Lint and format codebase', description: null, active_form: null, status: 'completed', source: 'codex', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 3200000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
        ],
        commit_count: 12,
        file_count: 8,
        sources: ['claude', 'codex', 'gemini'],
        last_archived_at: new Date(Date.now() - 3600000).toISOString(), // 1h ago
        enrichment_warnings: [],
      },
      {
        session_id: 'sess-bbb-222',
        started_at: new Date(Date.now() - 5 * 86400000).toISOString(),
        ended_at: new Date(Date.now() - 5 * 86400000 + 6120000).toISOString(),
        duration_ms: 6120000,
        tasks: [
          { id: '7', source_key: 'sess-bbb-222', subject: 'Implement session scanner', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 5 * 86400000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '8', source_key: 'sess-bbb-222', subject: 'Add idle detection', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 5 * 86400000 + 60000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '9', source_key: 'sess-bbb-222', subject: 'Wire up IPC commands', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 5 * 86400000 + 120000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
        ],
        commit_count: 7,
        file_count: 5,
        sources: ['claude'],
        last_archived_at: new Date(Date.now() - 5 * 86400000).toISOString(),
        enrichment_warnings: [],
      },
      {
        session_id: 'sess-ccc-333',
        started_at: new Date(Date.now() - 12 * 86400000).toISOString(),
        ended_at: new Date(Date.now() - 12 * 86400000 + 11100000).toISOString(),
        duration_ms: 11100000,
        tasks: [
          { id: '1', source_key: 'sess-ccc-333', subject: 'Set up project scaffold', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '2', source_key: 'sess-ccc-333', subject: 'Configure Tauri + Svelte', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 60000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '3', source_key: 'sess-ccc-333', subject: 'Add SQLite storage layer', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 120000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '4', source_key: 'sess-ccc-333', subject: 'Build sidebar and navigation', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 180000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '5', source_key: 'sess-ccc-333', subject: 'Implement git integration', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 240000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '6', source_key: 'sess-ccc-333', subject: 'Add file tree component', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 300000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: 'codex-0', source_key: 'sess-ccc-333', subject: 'Initialize project dependencies', description: null, active_form: null, status: 'completed', source: 'codex', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 360000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: 'codex-1', source_key: 'sess-ccc-333', subject: 'Set up CI pipeline', description: null, active_form: null, status: 'completed', source: 'codex', blocks: [], blocked_by: [], owner: null, archived_at: new Date(Date.now() - 12 * 86400000 + 420000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
        ],
        commit_count: 15,
        file_count: 12,
        sources: ['claude', 'codex'],
        last_archived_at: new Date(Date.now() - 12 * 86400000).toISOString(),
        enrichment_warnings: ['Could not resolve transcript time range for session sess-ccc-333; using task timestamp fallback.'],
      },
    ],
    errors: [],
  }))
}

/** Get files changed by a specific commit (for Git tab detail). */
export function getCommitFiles(projectPath, hash) {
  return invokeOrMock('get_commit_files', { projectPath, hash }, () => [
    { path: 'src/lib/GitTab.svelte', status: 'added' },
    { path: 'src/Shell.svelte', status: 'modified' },
    { path: 'src/lib/ipc.js', status: 'modified' },
    { path: 'src-tauri/src/git/commits.rs', status: 'modified' },
    { path: 'src-tauri/src/models/mod.rs', status: 'modified' },
  ])
}

/** Get diff hunks for a specific file in a specific commit (for inline diff view). */
export function getCommitDiff(projectPath, hash, filePath) {
  return invokeOrMock('get_commit_diff', { projectPath, hash, filePath }, () => MOCK_DIFF_HUNKS)
}

/** Get commits and files changed in a time range (for Git tab range view). */
export function getCommitsInRange(projectPath, after, before) {
  return invokeOrMock('get_commits_in_range', { projectPath, after, before }, () => ({
    commits: [
      { hash: 'abc12345', message: 'Add Git tab component', body: null, author: 'Developer', date: '2h' },
      { hash: 'def67890', message: 'Wire cross-tab navigation', body: null, author: 'Developer', date: '3h' },
      { hash: 'ghi11111', message: 'Add commit file detail view', body: null, author: 'Developer', date: '4h' },
    ],
    files: ['src/lib/GitTab.svelte', 'src/Shell.svelte', 'src/lib/ipc.js'],
  }))
}

/** Get aggregated activity stats for a project path. */
export function getProjectActivity(projectPath) {
  return invokeOrMock('get_project_activity', { projectPath }, () => ({
    total_active_ms: 0,
    total_duration_ms: 0,
    session_count: 0,
    last_session_at: null,
  }))
}

// ---------------------------------------------------------------------------
// Coordination IPC functions
// ---------------------------------------------------------------------------

/** Create a new coordination team. */
export function coordinationCreateTeam(teamName) {
  return invokeOrMock('coordination_create_team', { teamName }, () => undefined)
}

/** Disband a coordination team. */
export function coordinationDisbandTeam(teamName) {
  return invokeOrMock('coordination_disband_team', { teamName }, () => ({
    teamName,
    disbanded: true,
    alreadyDisbanded: false,
    message: 'team disbanded',
  }))
}

/** Add a member to an existing coordination team. */
export function coordinationAddMember(teamName, memberName, backendKind) {
  return invokeOrMock('coordination_add_member', { teamName, memberName, backendKind }, () => undefined)
}

/** Remove a member from an existing coordination team. */
export function coordinationRemoveMember(teamName, memberName) {
  return invokeOrMock('coordination_remove_member', { teamName, memberName }, () => undefined)
}

/** List all known coordination teams. */
export function coordinationListTeams() {
  return invokeOrMock('coordination_list_teams', undefined, () => [])
}

/** Get current team status (member list and runtime state summary). */
export function coordinationGetTeamStatus(teamName) {
  return invokeOrMock('coordination_get_team_status', { teamName }, () => ({
    teamName,
    members: [],
  }))
}

/** Initialize a team with lead/agent setup configuration. */
export function coordinationInitializeTeam(request) {
  return invokeOrMock('coordination_initialize_team', { request }, () => ({
    teamName: request?.teamName ?? '',
    succeededSteps: [
      'validate_configuration',
      'create_team',
      'create_panes',
      'launch_sessions',
      'join_mesh',
      'start_daemons',
      'send_onboarding',
    ],
    failedStep: null,
    retryable: false,
    message: 'team initialized',
    steps: [
      { step: 'validate_configuration', status: 'succeeded', message: 'request validated' },
      { step: 'create_team', status: 'succeeded', message: 'team created' },
      { step: 'send_onboarding', status: 'succeeded', message: 'onboarding messages sent' },
    ],
  }))
}

/** Hot-add one agent to an existing team. */
export function coordinationAddAgent(request) {
  return invokeOrMock('coordination_add_agent', { request }, () => ({
    teamName: request?.teamName ?? '',
    memberName: request?.agent?.name ?? '',
    succeededSteps: ['validate', 'create_pane', 'launch_session', 'join_mesh', 'start_daemon', 'send_onboarding', 'update_roster'],
    failedStep: null,
    retryable: false,
    message: 'agent added',
    steps: [
      { step: 'validate', status: 'succeeded', message: 'request validated' },
      { step: 'update_roster', status: 'succeeded', message: 'team roster updated' },
    ],
  }))
}

/** Re-send onboarding guidance to an existing team member. */
export function coordinationReonboard(teamName, memberName) {
  return invokeOrMock(
    'coordination_reonboard',
    { request: { teamName, memberName } },
    () => ({ delivered: true, method: 'tmux_injection' })
  )
}

/** Return mesh/tmux feature availability for UI gating. */
export function coordinationGetFeatureAvailability() {
  return invokeOrMock('coordination_get_feature_availability', undefined, () => ({
    canInitialize: true,
    meshAvailable: true,
    tmuxAvailable: true,
    blockingErrors: [],
  }))
}

/** Run preflight checks before team initialization. */
export function coordinationPreflightCheck(request) {
  return invokeOrMock('coordination_preflight_check', { request }, () => ({
    canInitialize: true,
    blockingErrors: [],
    agentWarnings: [],
  }))
}

/** Get live roster/runtime status for a running team. */
export function coordinationGetLiveTeamStatus(teamName) {
  return invokeOrMock('coordination_get_live_team_status', { teamName }, () => ({
    teamName,
    leadName: 'team-lead',
    members: [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: 'proj-core',
        description: 'Own orchestration',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'codex',
        model: 'gpt-5.3',
        projectId: 'proj-web',
        description: 'UI implementation',
        sessionStatus: 'idle',
        paneId: '%2',
      },
      {
        name: 'qa-reviewer',
        role: 'member',
        cliTool: 'gemini',
        model: 'gemini-2.5-pro',
        projectId: 'proj-core',
        description: 'Test coverage and code review',
        sessionStatus: 'active',
        paneId: '%3',
      },
    ],
  }))
}

/** Subscribe to orchestrator step progress events from the backend. */
export function onCoordinationStepProgress(callback) {
  return listen('coordination-step-progress', callback)
}

export function getDaemonStatus() {
  return invokeOrMock('get_daemon_status', undefined, () => ({
    status: 'connected',
    version: null,
    protocol_version: 0,
    expected_protocol_version: 0,
    uptime_secs: null,
    port: 17233,
    wsl_distro: null,
  }))
}

/** Get the current platform: "macos", "linux", or "windows". */
export function getPlatform() {
  return invokeOrMock('get_platform', undefined, () => 'linux')
}

/** Check daemon installation status (for wizard and update detection). */
export function checkDaemonInstallStatus() {
  return invokeOrMock('check_daemon_install_status', undefined, () => ({
    installed: true,
    version: '0.3.1',
    bundled_version: '0.3.1',
    needs_update: false,
    wsl_available: true,
    error: null,
  }))
}

/** Install (or update) daemon binary from bundled resources. */
export function installDaemon() {
  return invokeOrMock('install_daemon', undefined, () =>
    'Daemon installed successfully: taurhaus-daemon 0.3.1'
  )
}

/** Check mesh installation status for Mesh tab prerequisites. */
export function checkMeshInstallStatus() {
  return invokeOrMock('check_mesh_install_status', undefined, () => ({
    installed: true,
    version: '0.1.0',
    bundled_version: '0.1.0',
    needs_update: false,
    environment_available: true,
    error: null,
  }))
}

/** Install (or update) mesh binary from bundled resources. */
export function installMesh() {
  return invokeOrMock('install_mesh', undefined, () => 'Mesh installed successfully: mesh 0.1.0')
}
