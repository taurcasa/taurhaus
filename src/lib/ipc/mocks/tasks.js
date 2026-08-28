import { MOCK_DIFF_HUNKS } from './base.js'

export function buildMockProjectTasks() {
  return {
    tasks: [
      { id: '1', source_key: 'sess-aaa-111', subject: 'Add task scanner backend', description: 'Parse tasks from all three CLI tools', active_form: 'Adding task scanner', status: 'in_progress', source: 'claude', blocks: ['2'], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 300000).toISOString(), updated_at: new Date(Date.now() - 300000).toISOString() },
      { id: '2', source_key: 'sess-aaa-111', subject: 'Build TaskBoard UI component', description: null, active_form: null, status: 'pending', source: 'claude', blocks: [], blocked_by: ['1'], owner: null, state_changed_at: new Date(Date.now() - 3600000).toISOString(), updated_at: new Date(Date.now() - 3600000).toISOString() },
      { id: '3', source_key: 'sess-aaa-111', subject: 'Write integration tests', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 7200000).toISOString(), updated_at: new Date(Date.now() - 7200000).toISOString() },
      { id: 'codex-0', source_key: 'legacy-codex', subject: 'Initialize project structure', description: null, active_form: null, status: 'completed', source: 'codex', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 10800000).toISOString(), updated_at: new Date(Date.now() - 10800000).toISOString() },
      { id: 'codex-1', source_key: 'legacy-codex', subject: 'Implement CLI parsing', description: null, active_form: 'Implementing CLI parsing', status: 'in_progress', source: 'codex', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 900000).toISOString(), updated_at: new Date(Date.now() - 900000).toISOString() },
      { id: 'todo-1', source_key: 'agy-todo', subject: 'Write unit tests', description: null, active_form: null, status: 'pending', source: 'agy', blocks: [], blocked_by: [], owner: null, state_changed_at: new Date(Date.now() - 1800000).toISOString(), updated_at: new Date(Date.now() - 1800000).toISOString() },
    ],
    errors: [],
    // Backend task scanner contract includes per-source scan outcomes.
    source_outcomes: [],
  }
}

export function buildMockTaskDetail(taskId, source, sourceKey) {
  return {
    task: {
      id: taskId,
      source_key: sourceKey || (source === 'agy' ? 'agy-todo' : source === 'codex' ? 'legacy-codex' : 'legacy-claude'),
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
    ],
    files_changed: [
      'src-tauri/src/task_scanner/mod.rs',
      'src-tauri/src/task_scanner/types.rs',
    ],
  }
}

export function buildMockArchivedSessions() {
  const now = Date.now()
  return {
    sessions: [
      {
        session_id: 'sess-aaa-111',
        started_at: new Date(now - 2 * 86400000).toISOString(),
        ended_at: new Date(now - 2 * 86400000 + 8100000).toISOString(),
        duration_ms: 8100000,
        tasks: [
          { id: '10', source_key: 'sess-aaa-111', subject: 'Add task scanner backend', description: 'Parse tasks from all three CLI tools', active_form: null, status: 'completed', source: 'claude', blocks: ['11'], blocked_by: [], owner: null, archived_at: new Date(now - 3600000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
          { id: '11', source_key: 'sess-aaa-111', subject: 'Build TaskBoard UI component', description: null, active_form: null, status: 'completed', source: 'claude', blocks: [], blocked_by: ['10'], owner: null, archived_at: new Date(now - 3500000).toISOString(), archived_reason: 'completed_and_removed', last_status: 'completed' },
        ],
        commit_count: 12,
        file_count: 8,
        sources: ['claude', 'codex', 'agy'],
        last_archived_at: new Date(now - 3600000).toISOString(),
        enrichment_warnings: [],
      },
    ],
    errors: [],
  }
}

export function buildMockCommitFiles() {
  return [
    { path: 'src/lib/GitTab.svelte', status: 'added' },
    { path: 'src/Shell.svelte', status: 'modified' },
    { path: 'src/lib/ipc.js', status: 'modified' },
  ]
}

export function buildMockCommitDiff() {
  return MOCK_DIFF_HUNKS
}

export function buildMockCommitsInRange() {
  return {
    commits: [
      { hash: 'abc12345', message: 'Add Git tab component', body: null, author: 'Developer', date: '2h' },
      { hash: 'def67890', message: 'Wire cross-tab navigation', body: null, author: 'Developer', date: '3h' },
    ],
    files: ['src/lib/GitTab.svelte', 'src/Shell.svelte', 'src/lib/ipc.js'],
    truncated: false,
    total_count: 2,
  }
}
