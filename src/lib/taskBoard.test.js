/**
 * TaskBoard component tests.
 *
 * Tests the helper functions and the Kanban board rendering logic
 * including column grouping, tool icon selection, and edge states.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import { groupTasksByStatus, statusBadgeClass, statusLabel } from './taskHelpers.js'

const { eventListenMock, emitProjectTasksChanged } = vi.hoisted(() => {
  /** @type {{ event: string, handler: (event: any) => void }[]} */
  let handlers = []
  return {
    eventListenMock: vi.fn(async (event, cb) => {
      handlers.push({ event, handler: cb })
      return () => {
        handlers = handlers.filter((entry) => entry.handler !== cb)
      }
    }),
    emitProjectTasksChanged: (payload) => {
      handlers
        .filter((entry) => entry.event === 'project-tasks-changed')
        .forEach((entry) => entry.handler({ payload }))
    },
  }
})

// Mock markdown rendering (MarkdownRenderer depends on shiki/WASM)
vi.mock('./markdown.js', () => ({
  renderMarkdown: vi.fn((source) => Promise.resolve(
    source ? `<p>${source}</p>` : ''
  )),
}))

// ---------------------------------------------------------------------------
// Pure helper function tests
// ---------------------------------------------------------------------------

describe('statusBadgeClass', () => {
  it('returns green classes for in_progress', () => {
    expect(statusBadgeClass('in_progress')).toContain('success-400')
  })

  it('returns blue classes for pending', () => {
    expect(statusBadgeClass('pending')).toContain('info-400')
  })

  it('returns zinc classes for completed', () => {
    expect(statusBadgeClass('completed')).toContain('zinc-500')
  })

  it('renders stale exactly like completed, unlike the open statuses', () => {
    expect(statusBadgeClass('stale')).toBe(statusBadgeClass('completed'))
    expect(statusBadgeClass('stale')).not.toBe(statusBadgeClass('in_progress'))
    expect(statusBadgeClass('stale')).not.toBe(statusBadgeClass('pending'))
  })

  it('folds a future status token into the closed bucket, visibly', () => {
    const grouped = groupTasksByStatus([{ id: '9', status: 'unknown', subject: 'x' }])
    expect(grouped.completed.map((task) => task.id)).toEqual(['9'])
    expect(statusLabel('unknown')).toBe('Unknown')
  })
})

describe('statusLabel', () => {
  it('returns "In Progress" for in_progress', () => {
    expect(statusLabel('in_progress')).toBe('In Progress')
  })

  it('returns "Pending" for pending', () => {
    expect(statusLabel('pending')).toBe('Pending')
  })

  it('returns "Done" for completed', () => {
    expect(statusLabel('completed')).toBe('Done')
  })

  it('returns "Timed out" for stale', () => {
    expect(statusLabel('stale')).toBe('Timed out')
  })

  it('returns raw status for unknown values', () => {
    expect(statusLabel('cancelled')).toBe('cancelled')
  })
})

describe('groupTasksByStatus', () => {
  it('returns stable grouped reference for same task array input', () => {
    const tasks = [
      {
        id: '1',
        source: 'claude',
        source_key: 'default',
        status: 'pending',
        blocked_by: [],
      },
    ]

    const first = groupTasksByStatus(tasks)
    const second = groupTasksByStatus(tasks)

    expect(second).toBe(first)
    expect(second.pending).toHaveLength(1)
  })

  // Regression: b709a8ed exposed the backend's stale token to the board, but
  // exact three-status grouping dropped every timed-out task from all columns.
  it('keeps stale tasks reachable in the completed column', () => {
    const staleTask = {
      id: '42',
      source: 'claude',
      source_key: 'mesh-team',
      status: 'stale',
      blocked_by: [],
    }

    const grouped = groupTasksByStatus([staleTask])

    expect(grouped.completed).toEqual([staleTask])
  })
})

// ---------------------------------------------------------------------------
// Component rendering tests
// ---------------------------------------------------------------------------

// Mock the ipc module so we can control what getProjectTasks/getTaskDetail/getArchivedSessions return
vi.mock('./ipc.js', () => ({
  getProjectTasks: vi.fn(),
  getTaskDetail: vi.fn(),
  getArchivedSessions: vi.fn(),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: eventListenMock,
}))

// Import the mocks after vi.mock so we can control return values
const { getProjectTasks, getTaskDetail, getArchivedSessions } = await import('./ipc.js')

/** Default mock for getArchivedSessions — used in tests that switch to History tab. */
function mockArchivedSessions(sessions = []) {
  getArchivedSessions.mockResolvedValue({ sessions, errors: [] })
}

/** Helper to build a task with defaults. */
function makeTask(overrides = {}) {
  return {
    id: '1',
    source_key: 'claude-default',
    subject: 'Test task',
    description: null,
    active_form: null,
    status: 'pending',
    source: 'claude',
    blocks: [],
    blocked_by: [],
    owner: null,
    state_changed_at: null,
    updated_at: null,
    archived_at: null,
    archived_reason: null,
    last_status: null,
    ...overrides,
  }
}

function createDeferred() {
  /** @type {(value: any) => void} */
  let resolve
  /** @type {(reason?: any) => void} */
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

import TaskBoard from './TaskBoard.svelte'

describe('TaskBoard component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    delete window.__TAURI_INTERNALS__
  })

  it('shows loading skeleton before data arrives', () => {
    // Never resolve — keeps component in loading state
    getProjectTasks.mockReturnValue(new Promise(() => {}))

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    expect(screen.getByTestId('tasks-loading')).toBeTruthy()
  })

  it('shows empty state when no tasks', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [], errors: [] })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('tasks-empty')).toBeTruthy()
    })
  })

  it('ignores stale task fetch responses after project switch', async () => {
    const projectA = createDeferred()
    const projectB = createDeferred()
    getProjectTasks.mockImplementation((path) => {
      if (path === '/project-a') return projectA.promise
      if (path === '/project-b') return projectB.promise
      return Promise.resolve({ tasks: [], errors: [] })
    })

    const { rerender } = render(TaskBoard, {
      props: { projectPath: '/project-a', dark: false },
    })

    await waitFor(() => {
      expect(getProjectTasks).toHaveBeenCalledWith('/project-a')
    })

    await rerender({ projectPath: '/project-b', dark: false })
    await waitFor(() => {
      expect(getProjectTasks).toHaveBeenCalledWith('/project-b')
    })

    projectB.resolve({
      tasks: [makeTask({ id: 'b', source_key: 'codex-b', subject: 'Project B task', status: 'pending' })],
      errors: [],
    })
    await waitFor(() => {
      expect(screen.getByText('Project B task')).toBeTruthy()
    })

    projectA.resolve({
      tasks: [makeTask({ id: 'a', source_key: 'codex-a', subject: 'Project A task', status: 'pending' })],
      errors: [],
    })
    await waitFor(() => {
      expect(screen.getByText('Project B task')).toBeTruthy()
      expect(screen.queryByText('Project A task')).toBeNull()
    })
  })

  it('keeps rendered tasks visible during background refresh triggered by task-change event', async () => {
    window.__TAURI_INTERNALS__ = {}
    const deferred = createDeferred()
    getProjectTasks
      .mockResolvedValueOnce({
        tasks: [makeTask({ id: '1', subject: 'Initial task', status: 'pending' })],
        errors: [],
      })
      .mockReturnValueOnce(deferred.promise)

    render(TaskBoard, { props: { projectPath: '/test', projectId: 'proj-1', dark: false } })

    await waitFor(() => {
      expect(screen.getByText('Initial task')).toBeTruthy()
      expect(eventListenMock).toHaveBeenCalled()
    })

    emitProjectTasksChanged({ project_id: 'proj-1' })
    await waitFor(() => {
      expect(getProjectTasks).toHaveBeenCalledTimes(2)
    })

    expect(screen.queryByTestId('tasks-loading')).toBeNull()
    expect(screen.getByText('Initial task')).toBeTruthy()

    deferred.resolve({
      tasks: [makeTask({ id: '2', subject: 'Refreshed task', status: 'pending' })],
      errors: [],
    })

    await waitFor(() => {
      expect(screen.getByText('Refreshed task')).toBeTruthy()
    })
  })

  it('closes an open detail panel when the selected task disappears during a realtime refresh', async () => {
    window.__TAURI_INTERNALS__ = {}
    getProjectTasks
      .mockResolvedValueOnce({
        tasks: [makeTask({ id: '1', subject: 'Watched task', status: 'pending' })],
        errors: [],
      })
      .mockResolvedValueOnce({
        tasks: [makeTask({ id: '2', subject: 'Replacement task', status: 'pending' })],
        errors: [],
      })
    getTaskDetail.mockResolvedValue({
      task: makeTask({ id: '1', subject: 'Watched task', status: 'pending', description: 'detail body' }),
      session: null,
      commits: [],
      files_changed: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', projectId: 'proj-1', dark: false } })

    await waitFor(() => {
      expect(screen.getByText('Watched task')).toBeTruthy()
    })

    await fireEvent.click(screen.getByText('Watched task'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })

    emitProjectTasksChanged({ project_id: 'proj-1' })

    await waitFor(() => {
      expect(screen.queryByTestId('task-detail-panel')).toBeNull()
      expect(screen.getByText('Replacement task')).toBeTruthy()
    })
  })

  it('renders three kanban columns', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', subject: 'Active task', status: 'in_progress' }),
        makeTask({ id: '2', subject: 'Waiting task', status: 'pending' }),
        makeTask({ id: '3', subject: 'Done task', status: 'completed' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('kanban-column')).toHaveLength(3)
    })
  })

  it('renders task cards across columns', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', subject: 'Active task', status: 'in_progress' }),
        makeTask({ id: '2', subject: 'Waiting task', status: 'pending' }),
        makeTask({ id: '3', subject: 'Done task', status: 'completed' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('task-row')).toHaveLength(3)
    })

    expect(screen.getByText('Active task')).toBeTruthy()
    expect(screen.getByText('Waiting task')).toBeTruthy()
    expect(screen.getByText('Done task')).toBeTruthy()
  })

  it('sorts in-progress tasks by most recent state_changed_at', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: 'a', subject: 'Old in-progress', status: 'in_progress', state_changed_at: '2026-03-01T09:00:00Z' }),
        makeTask({ id: 'b', subject: 'Newest in-progress', status: 'in_progress', state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: 'c', subject: 'Middle in-progress', status: 'in_progress', state_changed_at: '2026-03-01T10:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const inProgressColumn = screen.getAllByTestId('kanban-column')[0]
    const taskRows = inProgressColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('Newest in-progress')
    expect(taskRows[1].textContent).toContain('Middle in-progress')
    expect(taskRows[2].textContent).toContain('Old in-progress')
  })

  it('uses stable identity tie-breaker for equal in-progress recency', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '2', subject: 'C', source: 'codex', source_key: 'k2', status: 'in_progress', state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'B', source: 'claude', source_key: 'k1', status: 'in_progress', state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'A', source: 'claude', source_key: 'k0', status: 'in_progress', state_changed_at: '2026-03-01T11:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const inProgressColumn = screen.getAllByTestId('kanban-column')[0]
    const taskRows = inProgressColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('A')
    expect(taskRows[1].textContent).toContain('B')
    expect(taskRows[2].textContent).toContain('C')
  })

  it('sorts pending by dependency count then recency', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: 'a', subject: 'Most blocked', status: 'pending', blocked_by: ['1', '2'], state_changed_at: '2026-03-01T08:00:00Z' }),
        makeTask({ id: 'b', subject: 'Recent single dependency', status: 'pending', blocked_by: ['1'], state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: 'c', subject: 'Older single dependency', status: 'pending', blocked_by: ['1'], state_changed_at: '2026-03-01T10:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const pendingColumn = screen.getAllByTestId('kanban-column')[1]
    const taskRows = pendingColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('Most blocked')
    expect(taskRows[1].textContent).toContain('Recent single dependency')
    expect(taskRows[2].textContent).toContain('Older single dependency')
  })

  it('uses stable identity tie-breaker for equal pending sort keys', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '2', subject: 'C', source: 'codex', source_key: 'k2', status: 'pending', blocked_by: ['x'], state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'B', source: 'claude', source_key: 'k1', status: 'pending', blocked_by: ['x'], state_changed_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'A', source: 'claude', source_key: 'k0', status: 'pending', blocked_by: ['x'], state_changed_at: '2026-03-01T11:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const pendingColumn = screen.getAllByTestId('kanban-column')[1]
    const taskRows = pendingColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('A')
    expect(taskRows[1].textContent).toContain('B')
    expect(taskRows[2].textContent).toContain('C')
  })

  it('sorts completed tasks by updated_at desc', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: 'a', subject: 'Old completed', status: 'completed', updated_at: '2026-03-01T08:00:00Z' }),
        makeTask({ id: 'b', subject: 'Newest completed', status: 'completed', updated_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: 'c', subject: 'Middle completed', status: 'completed', updated_at: '2026-03-01T10:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const completedColumn = screen.getAllByTestId('kanban-column')[2]
    const taskRows = completedColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('Newest completed')
    expect(taskRows[1].textContent).toContain('Middle completed')
    expect(taskRows[2].textContent).toContain('Old completed')
  })

  it('uses stable identity tie-breaker for equal completed timestamps', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '2', subject: 'C', source: 'codex', source_key: 'k2', status: 'completed', updated_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'B', source: 'claude', source_key: 'k1', status: 'completed', updated_at: '2026-03-01T11:00:00Z' }),
        makeTask({ id: '1', subject: 'A', source: 'claude', source_key: 'k0', status: 'completed', updated_at: '2026-03-01T11:00:00Z' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('kanban-column')).toHaveLength(3))
    const completedColumn = screen.getAllByTestId('kanban-column')[2]
    const taskRows = completedColumn.querySelectorAll('[data-testid="task-row"]')
    expect(taskRows).toHaveLength(3)
    expect(taskRows[0].textContent).toContain('A')
    expect(taskRows[1].textContent).toContain('B')
    expect(taskRows[2].textContent).toContain('C')
  })

  it('shows task count in Active sub-tab', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', status: 'pending' }),
        makeTask({ id: '2', status: 'completed', subject: 'Task B' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const activeTab = screen.getByTestId('sub-tab-active')
      expect(activeTab.textContent).toContain('2')
    })
  })

  it('renders tool icon with correct aria-label', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ source: 'codex', status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByLabelText('Codex')).toBeTruthy()
    })
  })

  it('shows description when present', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ description: 'Some detail here', status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('Some detail here')).toBeTruthy()
    })
  })

  it('shows active_form as secondary text for in-progress tasks', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'in_progress', active_form: 'Implementing parser', subject: 'Parser task' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('Parser task')).toBeTruthy()
      expect(screen.getByTestId('task-active-form').textContent).toContain('Implementing parser')
    })
  })

  it('shows blocked-by references', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending', blocked_by: ['3', '5'] })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('blocked by: #3, #5')).toBeTruthy()
    })
  })

  it('shows owner when present', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending', owner: 'researcher' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('researcher')).toBeTruthy()
    })
  })

  it('shows per-source error indicators', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [['codex', 'Failed to parse JSONL']],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('Codex: Failed to parse JSONL')).toBeTruthy()
    })
  })

  // Only Claude and Codex have task sources — `task_scanner/` has exactly
  // those two parsers, and agy/grok declare `transcript_parser: false`.
  it('renders a tool badge for each implemented task source', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', source: 'claude', status: 'pending' }),
        makeTask({ id: '2', source: 'codex', status: 'in_progress' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByLabelText('Claude')).toBeTruthy()
      expect(screen.getByLabelText('Codex')).toBeTruthy()
    })
  })

  it('applies line-through to completed task subjects', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ id: '1', subject: 'Finished work', status: 'completed' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const subject = screen.getByText('Finished work')
      expect(subject.className).toContain('line-through')
    })
  })

  it('shows column headers with correct labels', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('In Progress')).toBeTruthy()
      expect(screen.getByText('Pending')).toBeTruthy()
      expect(screen.getByText('Completed')).toBeTruthy()
    })
  })

  it('shows "No tasks" in empty columns', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'in_progress' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      // Two columns (pending + completed) should show "No tasks"
      const emptyLabels = screen.getAllByText('No tasks')
      expect(emptyLabels.length).toBe(2)
    })
  })

  // ---------------------------------------------------------------------------
  // Card selection + detail panel
  // ---------------------------------------------------------------------------

  it('shows the effort the lead assigned, with the reason on hover', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({
          status: 'in_progress',
          subject: 'Migrate the account store',
          effort: 'high',
          effort_why: 'the migration is irreversible',
        }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Migrate the account store')).toBeTruthy()
    })

    const chip = screen.getByTestId('task-effort')
    expect(chip.textContent).toContain('high')
    expect(chip.getAttribute('title')).toBe(
      'Task effort: high — the migration is irreversible'
    )
  })

  it('shows no effort chip for a task no lead assigned', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending', subject: 'Local todo' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Local todo')).toBeTruthy()
    })

    expect(screen.queryByTestId('task-effort')).toBeNull()
  })

  it('opens detail panel when card is clicked', async () => {
    const task = makeTask({ status: 'in_progress', subject: 'Clickable task' })
    getProjectTasks.mockResolvedValue({ tasks: [task], errors: [] })
    getTaskDetail.mockResolvedValue({
      task,
      session: null,
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Clickable task')).toBeTruthy()
    })

    // Click the card
    const card = screen.getByTestId('task-row')
    await fireEvent.click(card)

    // Detail panel should appear
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })
  })

  it('logs and surfaces a warning when task detail fetch fails', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const task = makeTask({ status: 'pending', subject: 'Broken detail task' })
    getProjectTasks.mockResolvedValue({ tasks: [task], errors: [] })
    getTaskDetail.mockRejectedValue(new Error('detail IPC failed'))

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Broken detail task')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('task-row'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
      expect(screen.getByTestId('task-detail-error').textContent).toContain(
        'Task detail failed to load. Showing basic task info.'
      )
    })

    expect(errorSpy).toHaveBeenCalledWith(
      expect.stringContaining('[tasks] failed to load task detail'),
      expect.any(Error)
    )
    errorSpy.mockRestore()
  })

  it('closes detail panel when close button is clicked', async () => {
    const task = makeTask({ status: 'pending', subject: 'Closable task' })
    getProjectTasks.mockResolvedValue({ tasks: [task], errors: [] })
    getTaskDetail.mockResolvedValue({
      task,
      session: null,
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Closable task')).toBeTruthy()
    })

    // Open panel
    await fireEvent.click(screen.getByTestId('task-row'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })

    // Close panel via close button
    await fireEvent.click(screen.getByTestId('detail-close'))
    await waitFor(() => {
      expect(screen.queryByTestId('task-detail-panel')).toBeNull()
    })
  })

  it('toggles detail panel when same card clicked twice', async () => {
    const task = makeTask({ status: 'pending', subject: 'Toggle task' })
    getProjectTasks.mockResolvedValue({ tasks: [task], errors: [] })
    getTaskDetail.mockResolvedValue({
      task,
      session: null,
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Toggle task')).toBeTruthy()
    })

    const card = screen.getByTestId('task-row')

    // First click opens
    await fireEvent.click(card)
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })

    // Second click closes
    await fireEvent.click(card)
    await waitFor(() => {
      expect(screen.queryByTestId('task-detail-panel')).toBeNull()
    })
  })

  it('no detail panel visible by default', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByTestId('task-row')).toBeTruthy()
    })
    expect(screen.queryByTestId('task-detail-panel')).toBeNull()
  })

  it('card has active press state class', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      const card = screen.getByTestId('task-row')
      expect(card.className).toContain('active:scale-[0.98]')
    })
  })

  it('closes detail panel when clicking board background', async () => {
    const task = makeTask({ status: 'pending', subject: 'Outside click test' })
    getProjectTasks.mockResolvedValue({ tasks: [task], errors: [] })
    getTaskDetail.mockResolvedValue({
      task,
      session: null,
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      expect(screen.getByText('Outside click test')).toBeTruthy()
    })

    // Open panel
    await fireEvent.click(screen.getByTestId('task-row'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })

    // Click on a column (board background, not a card)
    const column = screen.getAllByTestId('kanban-column')[0]
    await fireEvent.click(column)
    await waitFor(() => {
      expect(screen.queryByTestId('task-detail-panel')).toBeNull()
    })
  })

  // ---------------------------------------------------------------------------
  // Sub-tab switcher (Active / History)
  // ---------------------------------------------------------------------------

  it('renders Active and History sub-tabs', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [makeTask({ status: 'pending' })], errors: [] })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-active')).toBeTruthy()
      expect(screen.getByTestId('sub-tab-history')).toBeTruthy()
    })
  })

  it('defaults to Active sub-tab with Kanban visible', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-active').getAttribute('aria-selected')).toBe('true')
      expect(screen.getByTestId('sub-tab-history').getAttribute('aria-selected')).toBe('false')
      expect(screen.getAllByTestId('kanban-column')).toHaveLength(3)
    })
  })

  it('switches to History tab and hides Kanban', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })
    mockArchivedSessions([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('kanban-column')).toHaveLength(3)
    })

    await fireEvent.click(screen.getByTestId('sub-tab-history'))

    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-history').getAttribute('aria-selected')).toBe('true')
      expect(screen.getByTestId('sub-tab-active').getAttribute('aria-selected')).toBe('false')
      expect(screen.queryAllByTestId('kanban-column')).toHaveLength(0)
      expect(screen.getByTestId('history-tab-content')).toBeTruthy()
    })
  })

  it('switches back to Active tab from History', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })
    mockArchivedSessions([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('kanban-column')).toHaveLength(3)
    })

    // Switch to History
    await fireEvent.click(screen.getByTestId('sub-tab-history'))
    await waitFor(() => {
      expect(screen.queryAllByTestId('kanban-column')).toHaveLength(0)
    })

    // Switch back to Active
    await fireEvent.click(screen.getByTestId('sub-tab-active'))
    await waitFor(() => {
      expect(screen.getAllByTestId('kanban-column')).toHaveLength(3)
    })
  })

  it('shows task count in Active tab', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', status: 'pending' }),
        makeTask({ id: '2', status: 'completed', subject: 'Done' }),
        makeTask({ id: '3', status: 'in_progress', subject: 'Working' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const activeTab = screen.getByTestId('sub-tab-active')
      expect(activeTab.textContent).toContain('3')
    })
  })

  it('shows SessionHistory component in History tab', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [makeTask({ status: 'pending' })],
      errors: [],
    })
    mockArchivedSessions([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-history')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('sub-tab-history'))
    await waitFor(() => {
      expect(screen.getByTestId('history-tab-content')).toBeTruthy()
      // SessionHistory renders its own empty state
      expect(screen.getByTestId('history-empty')).toBeTruthy()
    })
  })

  it('sub-tabs have tablist role', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [makeTask({ status: 'pending' })], errors: [] })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-list').getAttribute('role')).toBe('tablist')
      expect(screen.getByTestId('sub-tab-active').getAttribute('role')).toBe('tab')
      expect(screen.getByTestId('sub-tab-history').getAttribute('role')).toBe('tab')
    })
  })

  it('supports arrow-key navigation between sub-tabs', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [makeTask({ status: 'pending' })], errors: [] })
    mockArchivedSessions([])
    const { fireEvent } = await import('@testing-library/svelte')

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })

    const activeTab = await screen.findByTestId('sub-tab-active')
    activeTab.focus()

    await fireEvent.keyDown(activeTab, { key: 'ArrowRight' })

    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByTestId('sub-tab-history'))
      expect(screen.getByTestId('sub-tab-history').getAttribute('aria-selected')).toBe('true')
    })
  })

  // ---------------------------------------------------------------------------
  // History → TaskDetailPanel integration
  // ---------------------------------------------------------------------------

  it('clicking a history task opens the detail panel', async () => {
    const historyTask = { id: '10', subject: 'Archived task one', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null }
    getProjectTasks.mockResolvedValue({ tasks: [makeTask({ status: 'pending' })], errors: [] })
    getArchivedSessions.mockResolvedValue({
      sessions: [{
        session_id: 'sess-aaa',
        started_at: '2026-02-20T10:00:00Z',
        ended_at: '2026-02-20T12:00:00Z',
        duration_ms: 7200000,
        tasks: [historyTask],
        commit_count: 3,
        file_count: 2,
        sources: ['claude'],
      }],
      errors: [],
    })
    getTaskDetail.mockResolvedValue({
      task: historyTask,
      session: { id: 'sess-aaa', started_at: '2026-02-20T10:00:00Z', ended_at: '2026-02-20T12:00:00Z' },
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-history')).toBeTruthy()
    })

    // Switch to History tab
    await fireEvent.click(screen.getByTestId('sub-tab-history'))
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    // Expand the session
    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByText('Archived task one')).toBeTruthy()
    })

    // Click the task
    await fireEvent.click(screen.getByTestId('history-task'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })
  })

  it('switching from History to Active closes the detail panel', async () => {
    const historyTask = { id: '10', subject: 'Archived task', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null }
    getProjectTasks.mockResolvedValue({ tasks: [makeTask({ status: 'pending' })], errors: [] })
    getArchivedSessions.mockResolvedValue({
      sessions: [{
        session_id: 'sess-aaa',
        started_at: '2026-02-20T10:00:00Z',
        ended_at: '2026-02-20T12:00:00Z',
        duration_ms: 7200000,
        tasks: [historyTask],
        commit_count: 3,
        file_count: 2,
        sources: ['claude'],
      }],
      errors: [],
    })
    getTaskDetail.mockResolvedValue({
      task: historyTask,
      session: null,
      commits: [],
      files_changed: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('sub-tab-history')).toBeTruthy()
    })

    // Switch to History, expand session, click task
    await fireEvent.click(screen.getByTestId('sub-tab-history'))
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })
    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('history-task')).toBeTruthy()
    })
    await fireEvent.click(screen.getByTestId('history-task'))
    await waitFor(() => {
      expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
    })

    // Switch back to Active — panel should close
    await fireEvent.click(screen.getByTestId('sub-tab-active'))
    await waitFor(() => {
      expect(screen.queryByTestId('task-detail-panel')).toBeNull()
    })
  })
})
