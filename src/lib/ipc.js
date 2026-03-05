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

/** Create and register a brand-new project under a parent directory. */
export function createProject(name, parentDir) {
  return invokeOrMock('create_project', { name, parentDir }, () => ({
    ...MOCK_DETAIL,
    name,
    path: `${parentDir.replace(/[\\/]+$/, '')}/${name}`,
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

const MOCK_ROLE_TEMPLATES = [
  {
    roleId: 'claude-orchestrator',
    name: 'Claude Orchestrator',
    version: '1.0.0',
    kind: 'lead',
    cliTool: 'claude',
    model: 'claude-opus-4-6',
    defaultNamePattern: 'lead-{project}',
    capabilities: ['planning', 'coordination', 'review', 'triage'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Coordinate team execution, assign scoped tasks, track blockers, and synthesize outcomes for the user.',
    behavioralContract: {
      communication: [
        'Acknowledge requests quickly and classify next action.',
        'Assign owners with acceptance criteria and expected evidence.',
      ],
      execution: [
        'Keep tasks scoped and verify completion evidence before closure.',
        'Enforce project conventions and quality gates.',
      ],
      escalation: [
        'Escalate blockers with context and options.',
        'Do not allow blocked work to stall silently.',
      ],
    },
    constraints: {
      minInstances: 1,
      maxInstances: 1,
      requiresLeadTool: null,
      allowedProjectBinding: 'lead_project',
    },
  },
  {
    roleId: 'codex-developer',
    name: 'Codex Developer',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'codex',
    model: 'gpt-5.3-codex',
    defaultNamePattern: 'dev-{n}',
    capabilities: ['implementation', 'testing', 'debugging'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Implement assigned scope with TDD where applicable, keep changes focused, and report verification steps.',
    behavioralContract: {
      communication: [
        'Acknowledge assignment and restate scope before editing.',
        'Provide concise progress updates on longer tasks.',
      ],
      execution: [
        'Keep edits scoped to assigned work.',
        'Write/update tests for behavior changes.',
      ],
      escalation: [
        'Escalate blockers immediately with attempted fixes.',
        'Flag unexpected repo state before continuing.',
      ],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 8,
      requiresLeadTool: 'claude',
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'claude-reviewer',
    name: 'Claude Reviewer',
    version: '1.0.0',
    kind: 'agent',
    cliTool: 'claude',
    model: 'claude-opus-4-6',
    defaultNamePattern: 'reviewer-{n}',
    capabilities: ['review', 'security', 'risk-analysis', 'testing'],
    builtIn: true,
    readOnly: true,
    instructions:
      'Review changes for correctness, regressions, security risk, and missing tests. Prioritize actionable findings.',
    behavioralContract: {
      communication: [
        'Confirm review scope before starting.',
        'Report findings ordered by severity with file references.',
      ],
      execution: [
        'Focus on behavior/regression risk over style nitpicks.',
        'Highlight residual risks when no critical findings exist.',
      ],
      escalation: [
        'Escalate high-risk defects immediately.',
      ],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 6,
      requiresLeadTool: 'claude',
      allowedProjectBinding: 'any',
    },
  },
  {
    roleId: 'custom-doc-writer',
    name: 'Documentation Writer',
    version: '0.2.0',
    kind: 'agent',
    cliTool: 'gemini',
    model: 'gemini-2.5-pro',
    defaultNamePattern: 'docs-{n}',
    capabilities: ['documentation', 'research'],
    builtIn: false,
    readOnly: false,
    instructions:
      'Produce concise documentation updates and cross-link architecture references.',
    behavioralContract: {
      communication: ['Share draft structure early for review.'],
      execution: ['Keep docs consistent with shipped behavior.'],
      escalation: ['Flag stale or conflicting docs as risks.'],
    },
    constraints: {
      minInstances: 0,
      maxInstances: 4,
      requiresLeadTool: null,
      allowedProjectBinding: 'any',
    },
  },
]

const MOCK_TEAM_PRESETS = [
  {
    presetId: 'fullstack-dev',
    name: 'Full Stack Dev Team',
    description: 'Claude orchestrator with two Codex developers.',
    version: '1.0.0',
    leadRoleId: 'claude-orchestrator',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      { roleId: 'codex-developer', count: 2, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'review-team',
    name: 'Review Team',
    description: 'Claude orchestrator with two parallel reviewers.',
    version: '1.0.0',
    leadRoleId: 'claude-orchestrator',
    builtIn: true,
    readOnly: true,
    agentSlots: [
      { roleId: 'claude-reviewer', count: 2, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-review-team', tmuxLayout: 'tiled' },
  },
  {
    presetId: 'docs-sprint',
    name: 'Docs Sprint Team',
    description: 'Lead plus one documentation-focused agent.',
    version: '0.2.0',
    leadRoleId: 'claude-orchestrator',
    builtIn: false,
    readOnly: false,
    agentSlots: [
      { roleId: 'custom-doc-writer', count: 1, projectBinding: 'lead_project', overrides: null },
    ],
    defaults: { teamNamePattern: '{project}-docs', tmuxLayout: 'even-horizontal' },
  },
]

const MOCK_TEMPLATE_STORAGE_STATUS = {
  mode: 'git',
  repoInitialized: true,
  dirty: true,
  pendingActions: [],
  lastCommit: Math.floor(Date.now() / 1000) - 3600,
}

const MOCK_TEMPLATE_HISTORY = [
  {
    commitId: 'f3b6c841d1f84b7e1a2c9018899f1f37f71aa001',
    shortId: 'f3b6c841',
    message: 'templates: tune claude reviewer rubric',
    author: 'taurhaus-dev-1',
    timestamp: Math.floor(Date.now() / 1000) - 600,
    changedPaths: ['roles/claude-reviewer.yaml'],
  },
  {
    commitId: 'de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322',
    shortId: 'de11ab00',
    message: 'templates: add docs sprint preset',
    author: 'taurhaus-dev-2',
    timestamp: Math.floor(Date.now() / 1000) - 2200,
    changedPaths: ['presets/docs-sprint.yaml', '_meta/state.json'],
  },
  {
    commitId: '9cc7fb70f1bf8da99ef8d8e50b179e744f5e6f10',
    shortId: '9cc7fb70',
    message: 'templates: introduce codex developer role',
    author: 'team-lead',
    timestamp: Math.floor(Date.now() / 1000) - 4800,
    changedPaths: ['roles/codex-developer.yaml'],
  },
]

const MOCK_TEMPLATE_DIFFS = {
  f3b6c841d1f84b7e1a2c9018899f1f37f71aa001: {
    commitId: 'f3b6c841d1f84b7e1a2c9018899f1f37f71aa001',
    files: [
      {
        path: 'roles/claude-reviewer.yaml',
        status: 'modified',
        hunks: [
          {
            old_start: 12,
            old_lines: 3,
            new_start: 12,
            new_lines: 4,
            lines: [
              { origin: ' ', old_lineno: 12, new_lineno: 12, content: 'behavioral_contract:' },
              {
                origin: '-',
                old_lineno: 13,
                new_lineno: null,
                content: '  execution: [focus on correctness]',
              },
              {
                origin: '+',
                old_lineno: null,
                new_lineno: 13,
                content: '  execution: [focus on correctness and regression risk]',
              },
              {
                origin: '+',
                old_lineno: null,
                new_lineno: 14,
                content: '  escalation: [raise high-risk findings immediately]',
              },
            ],
          },
        ],
      },
    ],
    stats: { filesChanged: 1, insertions: 2, deletions: 1 },
  },
  de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322: {
    commitId: 'de11ab008d1ca69f3f7a0b98b7f7c4d0f7d98322',
    files: [
      {
        path: 'presets/docs-sprint.yaml',
        status: 'added',
        hunks: [
          {
            old_start: 0,
            old_lines: 0,
            new_start: 1,
            new_lines: 5,
            lines: [
              { origin: '+', old_lineno: null, new_lineno: 1, content: 'preset_id: docs-sprint' },
              { origin: '+', old_lineno: null, new_lineno: 2, content: 'lead_role_id: claude-orchestrator' },
              { origin: '+', old_lineno: null, new_lineno: 3, content: 'agent_slots:' },
              { origin: '+', old_lineno: null, new_lineno: 4, content: '  - role_id: custom-doc-writer' },
              { origin: '+', old_lineno: null, new_lineno: 5, content: '    count: 1' },
            ],
          },
        ],
      },
    ],
    stats: { filesChanged: 1, insertions: 5, deletions: 0 },
  },
}

function roleTemplateSummary(template) {
  return {
    roleId: template.roleId,
    name: template.name,
    kind: template.kind,
    cliTool: template.cliTool,
    model: template.model,
    capabilities: template.capabilities ?? [],
    builtIn: Boolean(template.builtIn),
    readOnly: Boolean(template.readOnly),
  }
}

function teamPresetSummary(preset) {
  const referencedRoles = [
    preset.leadRoleId,
    ...(preset.agentSlots ?? []).map((slot) => slot.roleId),
  ]
    .map((roleId) => MOCK_ROLE_TEMPLATES.find((role) => role.roleId === roleId))
    .filter(Boolean)

  const tools = [...new Set(referencedRoles.map((role) => role.cliTool))]
  const capabilities = [...new Set(referencedRoles.flatMap((role) => role.capabilities ?? []))]

  return {
    presetId: preset.presetId,
    name: preset.name,
    description: preset.description,
    leadRoleId: preset.leadRoleId,
    roleCount: preset.agentSlots?.length ?? 0,
    agentCount: (preset.agentSlots ?? []).reduce((total, slot) => total + (slot.count ?? 0), 0),
    tools,
    capabilities,
    builtIn: Boolean(preset.builtIn),
    readOnly: Boolean(preset.readOnly),
  }
}

/** List role template summaries for the template catalog. */
export async function listRoleTemplates() {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core')
    const templates = await invoke('templates_list_roles_full')
    return (templates ?? []).map((template) => ({
      ...template,
      roleId: template?.roleId ?? template?.role_id ?? '',
      cliTool:
        template?.cliTool ??
        template?.cli_tool ??
        template?.defaults?.cliTool ??
        template?.defaults?.cli_tool ??
        null,
      model:
        template?.model ??
        template?.defaults?.model ??
        null,
      capabilities: Array.isArray(template?.capabilities) ? template.capabilities : [],
      builtIn: String(template?.source ?? '').toLowerCase() === 'built_in',
      readOnly: Boolean(template?.readOnly ?? template?.read_only),
    }))
  }
  return MOCK_ROLE_TEMPLATES.map(roleTemplateSummary)
}

/** Get one full role template by ID. */
export function getRoleTemplate(id) {
  return invokeOrMock('templates_get_role', { roleId: id }, () => {
    const template = MOCK_ROLE_TEMPLATES.find((entry) => entry.roleId === id)
    return template ? { ...template } : null
  })
}

function normalizeRoleTemplateInput(roleData) {
  const source =
    roleData && typeof roleData === 'object' && roleData.template
      ? roleData.template
      : roleData

  if (!source || typeof source !== 'object') {
    return source
  }

  const roleKind = String(source.kind ?? 'agent').toLowerCase() === 'lead' ? 'lead' : 'agent'
  const cliTool = String(
    source.tool ??
    source.cliTool ??
    source.cli_tool ??
    source.defaults?.cliTool ??
    source.defaults?.cli_tool ??
    (roleKind === 'lead' ? 'claude' : 'codex')
  ).toLowerCase()
  const model = String(
    source.model ??
    source.defaults?.model ??
    (cliTool === 'claude'
      ? 'claude-opus-4-6'
      : (cliTool === 'gemini' ? 'gemini-3.1-pro' : 'gpt-5.3-codex'))
  )
  const roleId = String(source.roleId ?? source.role_id ?? '').trim()
  const behavioralInput = source.behavioralContract ?? source.behavioral_contract
  const behavioralContract = Array.isArray(behavioralInput)
    ? {
      communication: [],
      execution: behavioralInput
        .map((entry) => {
          if (typeof entry === 'string') return entry.trim()
          if (!entry || typeof entry !== 'object') return ''
          if (entry.enabled === false) return ''
          return String(entry.rule ?? entry.text ?? '').trim()
        })
        .filter(Boolean),
      escalation: [],
    }
    : {
      communication: Array.isArray(behavioralInput?.communication)
        ? behavioralInput.communication.map((line) => String(line ?? '').trim()).filter(Boolean)
        : [],
      execution: Array.isArray(behavioralInput?.execution)
        ? behavioralInput.execution.map((line) => String(line ?? '').trim()).filter(Boolean)
        : [],
      escalation: Array.isArray(behavioralInput?.escalation)
        ? behavioralInput.escalation.map((line) => String(line ?? '').trim()).filter(Boolean)
        : [],
    }
  if (
    behavioralContract.communication.length === 0 &&
    behavioralContract.execution.length === 0 &&
    behavioralContract.escalation.length === 0
  ) {
    behavioralContract.execution = ['Execute assigned tasks and report status clearly.']
  }

  const capabilities = Array.isArray(source.capabilities)
    ? source.capabilities.map((capability) => String(capability ?? '').trim()).filter(Boolean)
    : []
  const constraints = source.constraints ?? {}
  const minInstancesRaw = Number(constraints.minInstances ?? constraints.min_instances ?? (roleKind === 'lead' ? 1 : 0))
  const maxInstancesRaw = Number(constraints.maxInstances ?? constraints.max_instances ?? (roleKind === 'lead' ? 1 : 8))
  const minInstances = Number.isFinite(minInstancesRaw) ? Math.max(0, Math.floor(minInstancesRaw)) : (roleKind === 'lead' ? 1 : 0)
  const maxInstances = Number.isFinite(maxInstancesRaw) ? Math.max(1, Math.floor(maxInstancesRaw)) : (roleKind === 'lead' ? 1 : 8)

  return {
    schema: {
      kind: 'role_template',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    roleId,
    name: String(source.name ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    kind: roleKind,
    defaults: {
      cliTool,
      model,
      defaultNamePattern: String(
        source.defaults?.defaultNamePattern ??
        source.defaults?.default_name_pattern ??
        (roleKind === 'lead' ? 'team-lead' : `${roleId || 'agent'}-{n}`)
      ),
    },
    instructions: String(source.instructions ?? '').trim(),
    behavioralContract,
    capabilities: capabilities.length > 0 ? capabilities : [roleKind === 'lead' ? 'orchestration' : 'implementation'],
    constraints: {
      minInstances: roleKind === 'lead' ? 1 : minInstances,
      maxInstances: roleKind === 'lead' ? 1 : Math.max(maxInstances, minInstances),
      requiresLeadTool: constraints.requiresLeadTool ?? constraints.requires_lead_tool ?? null,
      allowedProjectBinding:
        constraints.allowedProjectBinding ??
        constraints.allowed_project_binding ??
        'lead_project',
    },
  }
}

function normalizeTeamPresetInput(presetData) {
  const source =
    presetData && typeof presetData === 'object' && presetData.preset
      ? presetData.preset
      : presetData
  if (!source || typeof source !== 'object') {
    return source
  }

  const rawSlots = Array.isArray(source.agentSlots)
    ? source.agentSlots
    : (Array.isArray(source.agent_slots) ? source.agent_slots : [])
  const agentSlots = rawSlots.map((slot) => ({
    roleId: String(slot?.roleId ?? slot?.role_id ?? '').trim(),
    count: Math.max(1, Number(slot?.count ?? 1) || 1),
    projectBinding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
    projectId: slot?.projectId ?? slot?.project_id ?? null,
    overrides: slot?.overrides ?? null,
  }))

  return {
    schema: {
      kind: 'team_preset',
      version: Number(source.schema?.version ?? 1) || 1,
    },
    presetId: String(source.presetId ?? source.preset_id ?? '').trim(),
    name: String(source.name ?? '').trim(),
    description: String(source.description ?? '').trim(),
    version: String(source.version ?? '1.0.0'),
    leadRoleId: String(source.leadRoleId ?? source.lead_role_id ?? '').trim(),
    agentSlots,
    defaults: {
      teamNamePattern: String(
        source.defaults?.teamNamePattern ??
        source.defaults?.team_name_pattern ??
        '{project}-team'
      ),
      tmuxLayout: String(source.defaults?.tmuxLayout ?? source.defaults?.tmux_layout ?? 'tiled'),
    },
  }
}

/** Create or update a role template. */
export function upsertRoleTemplate(roleData) {
  const template = normalizeRoleTemplateInput(roleData)
  return invokeOrMock('templates_upsert_role', { request: { template } }, () => ({
    roleId: template?.roleId ?? template?.role_id ?? null,
    name: template?.name ?? '',
    kind: template?.kind ?? 'agent',
    builtIn: false,
    readOnly: false,
  }))
}

/** Delete a custom role template by ID. */
export function deleteRoleTemplate(roleId) {
  return invokeOrMock('templates_delete_role', { roleId }, () => ({
    roleId,
    deleted: true,
  }))
}

/** List team preset summaries for the template catalog. */
export async function listTeamPresets() {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core')
    const presets = await invoke('templates_list_presets_full')
    return (presets ?? []).map((preset) => {
      const leadRoleId = preset?.leadRoleId ?? preset?.lead_role_id ?? ''
      const agentSlots = Array.isArray(preset?.agentSlots ?? preset?.agent_slots)
        ? (preset?.agentSlots ?? preset?.agent_slots)
        : []

      return {
        ...preset,
        leadRoleId,
        roleCount: agentSlots.length,
        agentCount: agentSlots.reduce((total, slot) => total + (slot?.count ?? 0), 0),
        tools: Array.isArray(preset?.tools) ? preset.tools : [],
        capabilities: Array.isArray(preset?.capabilities) ? preset.capabilities : [],
        builtIn: String(preset?.source ?? '').toLowerCase() === 'built_in',
        readOnly: Boolean(preset?.readOnly ?? preset?.read_only),
      }
    })
  }
  return MOCK_TEAM_PRESETS.map(teamPresetSummary)
}

/** Get one full team preset by ID. */
export function getTeamPreset(id) {
  return invokeOrMock('templates_get_preset', { presetId: id }, () => {
    const preset = MOCK_TEAM_PRESETS.find((entry) => entry.presetId === id)
    return preset ? { ...preset } : null
  })
}

/** Create or update a team preset. */
export function upsertTeamPreset(presetData) {
  const preset = normalizeTeamPresetInput(presetData)
  return invokeOrMock('templates_upsert_preset', { request: { preset } }, () => ({
    presetId: preset?.presetId ?? preset?.preset_id ?? null,
    name: preset?.name ?? '',
    leadRoleId: preset?.leadRoleId ?? preset?.lead_role_id ?? '',
    agentSlots: Array.isArray(preset?.agentSlots)
      ? preset.agentSlots
      : (Array.isArray(preset?.agent_slots) ? preset.agent_slots : []),
    builtIn: false,
    readOnly: false,
  }))
}

/** Delete a custom team preset by ID. */
export function deleteTeamPreset(presetId) {
  return invokeOrMock('templates_delete_preset', { presetId }, () => ({
    presetId,
    deleted: true,
  }))
}

/** Compose a team from role/preset selections and overrides. */
export function composeTeam(request) {
  const normalizedAgentSlots = (request?.agentSlots ?? request?.agent_slots ?? []).map((slot) => ({
    role_id: slot?.roleId ?? slot?.role_id ?? '',
    count: Number(slot?.count ?? 0),
    project_binding: slot?.projectBinding ?? slot?.project_binding ?? 'lead_project',
    project_id: slot?.projectId ?? slot?.project_id ?? null,
    overrides: slot?.overrides ?? null,
  }))

  const normalizedRequest = {
    leadRoleId: request?.leadRoleId ?? request?.lead_role_id ?? '',
    agentSlots: normalizedAgentSlots,
    overrides: {
      ...(request?.overrides ?? {}),
      ...(request?.projectName ? { projectName: request.projectName } : {}),
    },
  }

  return invokeOrMock('templates_compose_team', { request: normalizedRequest }, () => {
    const leadName = request?.projectName ? `lead-${request.projectName}` : 'lead-project'
    return {
      roster: [
        {
          name: leadName,
          roleId: 'claude-orchestrator',
          roleKind: 'lead',
          cliTool: 'claude',
          model: 'claude-opus-4-6',
          instructions: 'Coordinate execution and unblock the team.',
          behavioralContract: {
            communication: ['Acknowledge assignments quickly.'],
            execution: ['Delegate scoped tasks and verify completion evidence.'],
            escalation: ['Escalate blockers immediately.'],
          },
          capabilities: ['planning', 'coordination'],
          projectBinding: 'lead_project',
          projectId: null,
        },
      ],
      warnings: normalizedAgentSlots.length ? [] : ['No agent slots selected; roster includes lead only.'],
      validationErrors: [],
    }
  }).catch((error) => {
    const message =
      error?.message
      || (typeof error === 'string' ? error : '')
      || (error ? JSON.stringify(error) : '')
      || 'templates_compose_team failed'
    throw new Error(message)
  })
}

/** Get template storage health/status for history and dirty indicators. */
export function getTemplateStorageStatus() {
  return invokeOrMock('templates_get_storage_status', undefined, () => ({
    ...MOCK_TEMPLATE_STORAGE_STATUS,
  }))
}

/** Get template history commits across all managed templates. */
export function getTemplateHistory(limit = 50, cursor = null) {
  return invokeOrMock('templates_get_history', { limit, cursor }, () => {
    const page = (MOCK_TEMPLATE_HISTORY ?? []).slice(0, Math.max(1, Math.min(200, limit || 50)))
    return { commits: page, nextCursor: null }
  })
}

/** Get a template-only diff for one commit in template history. */
export function getTemplateDiff(commitId) {
  return invokeOrMock('templates_get_diff', { commitId }, () => {
    return (
      MOCK_TEMPLATE_DIFFS[commitId] ?? {
        commitId,
        files: [],
        stats: { filesChanged: 0, insertions: 0, deletions: 0 },
      }
    )
  })
}

/** Revert one template id to a historical commit. */
export function revertTemplateVersion(id, commitHash) {
  return invokeOrMock('templates_revert', { request: { id, commitHash } }, () => undefined)
}

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
  return invokeOrMock('coordination_remove_member', { teamName, memberName }, () => ({
    teamName,
    memberName,
    removed: true,
    message: 'member removed',
    steps: [],
    warnings: [],
  }))
}

/** List all known coordination teams. */
export function coordinationListTeams() {
  return invokeOrMock('coordination_list_teams', undefined, () => ({ teams: [], warnings: [] }))
}

/** Get current team status (member list and runtime state summary). */
export function coordinationGetTeamStatus(teamName) {
  return invokeOrMock('coordination_get_team_status', { teamName }, () => ({
    teamName,
    members: [],
  }))
}

function normalizeBehavioralContractPayload(value) {
  if (!value || typeof value !== 'object') return null
  return {
    communication: Array.isArray(value.communication) ? value.communication : [],
    execution: Array.isArray(value.execution) ? value.execution : [],
    escalation: Array.isArray(value.escalation) ? value.escalation : [],
  }
}

function normalizeAgentSetupPayload(value) {
  if (!value || typeof value !== 'object') return value
  return {
    ...value,
    roleId: value?.roleId ?? value?.role_id ?? null,
    instructions: value?.instructions ?? null,
    behavioralContract: normalizeBehavioralContractPayload(
      value?.behavioralContract ?? value?.behavioral_contract
    ),
    capabilities: Array.isArray(value?.capabilities) ? value.capabilities : null,
  }
}

function normalizeInitializeTeamPayload(request) {
  if (!request || typeof request !== 'object') return request
  return {
    ...request,
    lead: normalizeAgentSetupPayload(request.lead),
    agents: Array.isArray(request.agents)
      ? request.agents.map((agent) => normalizeAgentSetupPayload(agent))
      : [],
  }
}

/** Initialize a team with lead/agent setup configuration. */
export function coordinationInitializeTeam(request) {
  const payload = normalizeInitializeTeamPayload(request)
  return invokeOrMock('coordination_initialize_team', { request: payload }, () => ({
    teamName: payload?.teamName ?? '',
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

/** Resume one offline member in an existing team. */
export function coordinationResumeMember(teamName, memberName, contextMode = 'continue') {
  return invokeOrMock(
    'coordination_resume_member',
    { request: { teamName, memberName, contextMode } },
    () => ({
      teamName,
      memberName,
      resumed: true,
      succeededSteps: ['validate', 'resolve_pane', 'launch_session', 'update_runtime'],
      failedStep: null,
      retryable: false,
      message: 'member resumed',
      steps: [
        { step: 'validate', status: 'succeeded', message: 'request validated' },
        { step: 'update_runtime', status: 'succeeded', message: 'runtime updated' },
      ],
      warnings: [],
      paneId: '%2',
      reusedPane: false,
    })
  )
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
