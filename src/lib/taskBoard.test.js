/**
 * TaskBoard component tests.
 *
 * Tests the helper functions and the component rendering logic
 * including status grouping, tool icon selection, and edge states.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import { statusBadgeClass, statusLabel } from './taskHelpers.js'

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

// Mock the ipc module so we can control what getProjectTasks returns
vi.mock('./ipc.js', () => ({
  getProjectTasks: vi.fn(),
}))

// Import the mock after vi.mock so we can control return values
const { getProjectTasks } = await import('./ipc.js')

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

  it('renders task rows grouped by status', async () => {
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

    // Check all three task subjects render
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
})
