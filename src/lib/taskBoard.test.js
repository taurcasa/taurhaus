/**
 * TaskBoard component tests.
 *
 * Tests the helper functions and the Kanban board rendering logic
 * including column grouping, tool icon selection, and edge states.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import { statusBadgeClass, statusLabel } from './taskHelpers.js'

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

  it('returns zinc classes for unknown status', () => {
    expect(statusBadgeClass('unknown')).toContain('zinc-500')
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

  it('returns raw status for unknown values', () => {
    expect(statusLabel('cancelled')).toBe('cancelled')
  })
})

// ---------------------------------------------------------------------------
// Component rendering tests
// ---------------------------------------------------------------------------

// Mock the ipc module so we can control what getProjectTasks/getTaskDetail return
vi.mock('./ipc.js', () => ({
  getProjectTasks: vi.fn(),
  getTaskDetail: vi.fn(),
}))

// Import the mocks after vi.mock so we can control return values
const { getProjectTasks, getTaskDetail } = await import('./ipc.js')

/** Helper to build a task with defaults. */
function makeTask(overrides = {}) {
  return {
    id: '1',
    subject: 'Test task',
    description: null,
    active_form: null,
    status: 'pending',
    source: 'claude',
    blocks: [],
    blocked_by: [],
    owner: null,
    ...overrides,
  }
}

import TaskBoard from './TaskBoard.svelte'

describe('TaskBoard component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
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

  it('shows task count in header', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', status: 'pending' }),
        makeTask({ id: '2', status: 'completed', subject: 'Task B' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('2 tasks')).toBeTruthy()
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

  it('renders all three tool types', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: [
        makeTask({ id: '1', source: 'claude', status: 'pending' }),
        makeTask({ id: '2', source: 'codex', status: 'in_progress' }),
        makeTask({ id: '3', source: 'gemini', status: 'completed' }),
      ],
      errors: [],
    })

    render(TaskBoard, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByLabelText('Claude')).toBeTruthy()
      expect(screen.getByLabelText('Codex')).toBeTruthy()
      expect(screen.getByLabelText('Gemini')).toBeTruthy()
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
})
