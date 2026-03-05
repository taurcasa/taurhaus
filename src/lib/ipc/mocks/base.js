/**
 * Shared mock fixtures for Vite-only frontend mode.
 */

export const MOCK_PROJECTS = [
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

export const MOCK_COMMITS = [
  { hash: 'abc12345', message: 'Add new feature', body: 'Implemented the session history view with accordion\nlayout and drill-down navigation.', author: 'Developer', date: '2h', timestamp: Math.floor(Date.now() / 1000) - 7200 },
  { hash: 'def67890', message: 'Fix bug in parser', body: null, author: 'Developer', date: '5h', timestamp: Math.floor(Date.now() / 1000) - 18000 },
  { hash: 'ghi11111', message: 'Update dependencies', body: null, author: 'Developer', date: '1d', timestamp: Math.floor(Date.now() / 1000) - 86400 },
]

export const MOCK_DIFF_HUNKS = [
  {
    old_start: 1,
    old_lines: 5,
    new_start: 1,
    new_lines: 7,
    lines: [
      { origin: ' ', content: 'import { onMount } from "svelte"', old_lineno: 1, new_lineno: 1 },
      { origin: ' ', content: '', old_lineno: 2, new_lineno: 2 },
      { origin: '-', content: 'const count = 0', old_lineno: 3, new_lineno: null },
      { origin: '+', content: 'let count = $state(0)', old_lineno: null, new_lineno: 3 },
      { origin: '+', content: 'let doubled = $derived(count * 2)', old_lineno: null, new_lineno: 4 },
      { origin: ' ', content: '', old_lineno: 4, new_lineno: 5 },
      { origin: ' ', content: 'function increment() {', old_lineno: 5, new_lineno: 6 },
    ],
  },
]

export const MOCK_FILE_TREE = [
  {
    name: 'src',
    path: 'src',
    is_dir: true,
    children: [
      { name: 'main.rs', path: 'src/main.rs', is_dir: false, children: [] },
      { name: 'lib.rs', path: 'src/lib.rs', is_dir: false, children: [] },
    ],
  },
  { name: 'Cargo.toml', path: 'Cargo.toml', is_dir: false, children: [] },
  { name: 'README.md', path: 'README.md', is_dir: false, children: [] },
]

export const MOCK_SESSION = {
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

export const MOCK_SESSIONS = [
  { id: 'mock-session-1', project_id: 'mock-1', date: new Date(Date.now() - 2 * 86400000).toISOString(), summary: 'Completed Phase 5B — git module, file reader, Overview and Files tabs.' },
  { id: 'mock-session-2', project_id: 'mock-1', date: new Date(Date.now() - 5 * 86400000).toISOString(), summary: 'Completed Phase 5A — scaffold, SQLite, project CRUD.' },
  { id: 'mock-session-3', project_id: 'mock-1', date: new Date(Date.now() - 10 * 86400000).toISOString(), summary: 'Architecture decisions — 22 ADRs across 6 topics.' },
]

export const MOCK_DETAIL = {
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

export const MOCK_SEARCH_RESULTS = [
  { project_id: 'mock-1', entity_type: 'document', file_path: 'README.md', title: 'README', snippet: 'Desktop tool for AI project management', relevance_score: 1.5 },
  { project_id: 'mock-1', entity_type: 'session', file_path: 'session:s1', title: 'Phase 5B Complete', snippet: 'Completed git module and file reader', relevance_score: 1.2 },
  { project_id: 'mock-1', entity_type: 'commit', file_path: 'commit:abc123', title: 'Add tantivy search', snippet: 'Implement full-text search with BM25', relevance_score: 0.8 },
]

export const MOCK_RELATIONSHIPS = [
  { id: 'rel-1', source_project_id: 'mock-1', target_project_id: 'mock-3', relationship_type: 'references', detection_source: 'claude_md', dismissed: false, first_detected_at: '2026-01-15T00:00:00Z', last_seen_at: '2026-02-17T00:00:00Z' },
  { id: 'rel-2', source_project_id: 'mock-1', target_project_id: 'mock-4', relationship_type: 'references', detection_source: 'claude_md', dismissed: false, first_detected_at: '2026-01-15T00:00:00Z', last_seen_at: '2026-02-17T00:00:00Z' },
]

export const MOCK_SETTINGS = {
  scan_directories: ['~/projects'],
  thresholds: { active_days: 7, recent_days: 30, stale_days: 90 },
  ignore_patterns: ['node_modules', '.git', 'target', 'dist'],
  code_theme: { light: 'github-light', dark: 'github-dark-dimmed' },
  terminal: { emulator: 'windows_terminal', custom_command: '', tmux_layout: 'new_window', cli_commands: {} },
  daemon: { port: 17233, path: '~/.local/bin/taurhaus-daemon', auto_start: true },
  dark_mode: false,
}

export const MOCK_CLAUDE_SESSIONS = [
  {
    pid: 12345,
    project_path: '~/projects/taurhaus',
    tty: '/dev/pts/2',
    args: 'claude --dangerously-skip-permissions --continue',
    cli_tool: 'claude',
    tmux_session: 'taurhaus',
    tmux_window: '1',
    tmux_pane: '%3',
    tmux_window_name: 'taurhaus',
    state: 'active',
    session_id: 'abc-123-def',
    jsonl_path: '/home/user/.claude/projects/-home-user-projects-taurhaus/abc-123-def.jsonl',
  },
  {
    pid: 12350,
    project_path: '~/projects/taurhaus',
    tty: '/dev/pts/5',
    args: 'codex --yolo',
    cli_tool: 'codex',
    tmux_session: 'taurhaus',
    tmux_window: '1',
    tmux_pane: '%8',
    tmux_window_name: 'taurhaus',
    state: 'idle',
    session_id: 'codex-uuid-1234',
    jsonl_path: '/home/user/.codex/sessions/2026/02/21/rollout-2026-02-21T10-00-00-codex-uuid-1234.jsonl',
  },
  {
    pid: 12346,
    project_path: '~/projects/mir',
    tty: '/dev/pts/4',
    args: 'claude --dangerously-skip-permissions',
    cli_tool: 'claude',
    tmux_session: 'taurhaus',
    tmux_window: '3',
    tmux_pane: '%7',
    tmux_window_name: 'mir',
    state: 'idle',
    session_id: 'def-456-ghi',
    jsonl_path: '/home/user/.claude/projects/-home-user-projects-mir/def-456-ghi.jsonl',
  },
  {
    pid: 12360,
    project_path: '~/projects/taurui',
    tty: '/dev/pts/6',
    args: 'gemini --yolo',
    cli_tool: 'gemini',
    tmux_session: 'taurhaus',
    tmux_window: '4',
    tmux_pane: '%10',
    tmux_window_name: 'taurui',
    state: 'active',
    session_id: 'gemini-sess-5678',
    jsonl_path: null,
  },
]
