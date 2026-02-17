/**
 * IPC wrapper — thin layer over Tauri invoke() with mock fallback.
 *
 * When running inside Tauri (`just dev`), calls the Rust backend.
 * When running in Vite-only mode (`just dev-frontend`), returns mock data
 * so frontend development works without the Rust backend.
 */

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
// Mock data (used in Vite-only mode)
// ---------------------------------------------------------------------------

const MOCK_PROJECTS = [
  { id: 'mock-1', name: 'taurhaus', path: '~/projects/taurhaus', activity_state: 'active', last_activity_at: new Date().toISOString(), branch: 'main', is_dirty: false },
  { id: 'mock-2', name: 'missing_invoice_reloaded', path: '~/projects/mir', activity_state: 'active', last_activity_at: new Date().toISOString(), branch: 'feat/auth', is_dirty: true },
  { id: 'mock-3', name: 'taurui', path: '~/projects/taurui', activity_state: 'active', last_activity_at: new Date().toISOString(), branch: 'main', is_dirty: false },
  { id: 'mock-4', name: 'taursec', path: '~/projects/taursec', activity_state: 'recent', last_activity_at: null, branch: 'main', is_dirty: false },
  { id: 'mock-5', name: 'taursult', path: '~/projects/taursult', activity_state: 'recent', last_activity_at: null, branch: 'main', is_dirty: false },
  { id: 'mock-6', name: 'ledger', path: '~/projects/ledger', activity_state: 'stale', last_activity_at: null, branch: 'main', is_dirty: false },
  { id: 'mock-7', name: 'aitx', path: '~/projects/aitx', activity_state: 'stale', last_activity_at: null, branch: 'main', is_dirty: false },
  { id: 'mock-8', name: 'taurmolt', path: '~/projects/taurmolt', activity_state: 'dormant', last_activity_at: null, branch: 'main', is_dirty: false },
  { id: 'mock-9', name: 'taurora', path: '~/projects/taurora', activity_state: 'dormant', last_activity_at: null, branch: 'develop', is_dirty: false },
  { id: 'mock-10', name: 'taurox', path: '~/projects/taurox', activity_state: 'dormant', last_activity_at: null, branch: 'main', is_dirty: false },
]

const MOCK_COMMITS = [
  { hash: 'abc12345', message: 'Add new feature', author: 'Developer', date: '2h' },
  { hash: 'def67890', message: 'Fix bug in parser', author: 'Developer', date: '5h' },
  { hash: 'ghi11111', message: 'Update dependencies', author: 'Developer', date: '1d' },
]

const MOCK_FILE_TREE = [
  { name: 'src', path: 'src', is_dir: true, children: [
    { name: 'main.rs', path: 'src/main.rs', is_dir: false, children: [] },
    { name: 'lib.rs', path: 'src/lib.rs', is_dir: false, children: [] },
  ]},
  { name: 'Cargo.toml', path: 'Cargo.toml', is_dir: false, children: [] },
  { name: 'README.md', path: 'README.md', is_dir: false, children: [] },
]

const MOCK_SESSION = {
  id: 'mock-session-1',
  project_id: 'mock-1',
  date: new Date(Date.now() - 2 * 86400000).toISOString(),
  summary: 'Completed Phase 5B implementation — git module, file reader, Overview and Files tabs.',
  next_steps: ['Implement file watcher', 'Add session import pipeline', 'Build session display in Overview tab'],
  open_questions: ['Virtual scrolling for large commit lists'],
  metadata: { branch: 'main', commit_range: 'abc123..def456' },
  file_path: 'docs/sessions/session-2026-02-15T14-30-45.md',
  created_at: new Date(Date.now() - 2 * 86400000).toISOString(),
}

const MOCK_SESSIONS = [
  { id: 'mock-session-1', project_id: 'mock-1', date: new Date(Date.now() - 2 * 86400000).toISOString(), summary: 'Completed Phase 5B — git module, file reader, Overview and Files tabs.' },
  { id: 'mock-session-2', project_id: 'mock-1', date: new Date(Date.now() - 5 * 86400000).toISOString(), summary: 'Completed Phase 5A — scaffold, SQLite, project CRUD.' },
  { id: 'mock-session-3', project_id: 'mock-1', date: new Date(Date.now() - 10 * 86400000).toISOString(), summary: 'Architecture decisions — 22 ADRs across 6 topics.' },
]

const MOCK_DETAIL = {
  id: 'mock-1',
  name: 'taurhaus',
  path: '~/projects/taurhaus',
  description: 'Desktop tool for AI project management',
  activity_state: 'active',
  last_activity_at: new Date().toISOString(),
  hero_preference: null,
  created_at: '2025-01-01T00:00:00Z',
  updated_at: new Date().toISOString(),
  branch: 'main',
  is_dirty: false,
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
