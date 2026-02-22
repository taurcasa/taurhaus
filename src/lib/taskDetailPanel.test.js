/**
 * TaskDetailPanel component tests.
 *
 * Tests progressive disclosure (sections appear/disappear based on data),
 * loading state, sparse task rendering, and close behavior.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/svelte'
import TaskDetailPanel from './TaskDetailPanel.svelte'

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** A full task with all fields populated. */
const FULL_TASK = {
  id: '1',
  subject: 'Add task scanner backend',
  description: 'Parse tasks from all three CLI tools',
  active_form: 'Adding task scanner',
  status: 'in_progress',
  source: 'claude',
  blocks: ['2', '3'],
  blocked_by: ['0'],
  owner: 'researcher',
  session_id: 'abc-123-def',
}

/** A sparse task (Gemini TODO item — only subject + status). */
const SPARSE_TASK = {
  id: 'todo-5',
  subject: 'Write unit tests',
  description: null,
  active_form: null,
  status: 'pending',
  source: 'gemini',
  blocks: [],
  blocked_by: [],
  owner: null,
  session_id: null,
}

/** Full detail response with all sections populated. */
const FULL_DETAIL = {
  task: FULL_TASK,
  session: {
    id: 'abc-123-def-456-ghi',
    started_at: '2026-02-22T03:59:01.775Z',
    ended_at: '2026-02-22T04:30:00.000Z',
  },
  commits: [
    { hash: 'abc12345', message: 'Add task scanner types', author: 'Dev', date: '30m' },
    { hash: 'def67890', message: 'Implement Claude parser', author: 'Dev', date: '1h' },
  ],
  files_changed: [
    'src-tauri/src/task_scanner/mod.rs',
    'src-tauri/src/task_scanner/claude.rs',
    'src/lib/TaskBoard.svelte',
  ],
}

/** Sparse detail (Gemini task — no session, commits, or files). */
const SPARSE_DETAIL = {
  task: SPARSE_TASK,
  session: null,
  commits: [],
  files_changed: [],
}

function renderPanel(props = {}) {
  return render(TaskDetailPanel, {
    task: FULL_TASK,
    detail: FULL_DETAIL,
    dark: true,
    onClose: vi.fn(),
    ...props,
  })
}

// ---------------------------------------------------------------------------
// Component rendering tests
// ---------------------------------------------------------------------------

describe('TaskDetailPanel', () => {
  it('renders the panel with testid', () => {
    renderPanel()
    expect(screen.getByTestId('task-detail-panel')).toBeTruthy()
  })

  it('shows task subject in header', () => {
    renderPanel()
    expect(screen.getByText('Add task scanner backend')).toBeTruthy()
  })

  it('shows source tool label', () => {
    renderPanel()
    expect(screen.getByText('Claude')).toBeTruthy()
  })

  it('shows status badge', () => {
    renderPanel()
    expect(screen.getByText('In Progress')).toBeTruthy()
  })

  it('shows close button', () => {
    renderPanel()
    expect(screen.getByTestId('detail-close')).toBeTruthy()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    renderPanel({ onClose })
    await fireEvent.click(screen.getByTestId('detail-close'))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('calls onClose when Escape pressed', async () => {
    const onClose = vi.fn()
    renderPanel({ onClose })
    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledOnce()
  })
})

describe('Loading state', () => {
  it('shows loading skeleton when detail is null', () => {
    renderPanel({ detail: null })
    expect(screen.getByTestId('detail-loading')).toBeTruthy()
  })

  it('hides loading when detail is provided', () => {
    renderPanel()
    expect(screen.queryByTestId('detail-loading')).toBeNull()
  })
})

describe('Description section', () => {
  it('shows description when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-description')).toBeTruthy()
    expect(screen.getByText('Parse tasks from all three CLI tools')).toBeTruthy()
  })

  it('hides description section for sparse task', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-description')).toBeNull()
  })
})

describe('Session section', () => {
  it('shows session info when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-session')).toBeTruthy()
    // Truncated UUID
    expect(screen.getByText('abc-123-')).toBeTruthy()
  })

  it('hides session section when null', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-session')).toBeNull()
  })
})

describe('Commits section', () => {
  it('shows commits when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-commits')).toBeTruthy()
    expect(screen.getByText('Commits (2)')).toBeTruthy()
    expect(screen.getByText('abc12345')).toBeTruthy()
    expect(screen.getByText('def67890')).toBeTruthy()
  })

  it('shows commit messages', () => {
    renderPanel()
    expect(screen.getByText('Add task scanner types')).toBeTruthy()
    expect(screen.getByText('Implement Claude parser')).toBeTruthy()
  })

  it('hides commits section when empty', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-commits')).toBeNull()
  })
})

describe('Files Changed section', () => {
  it('shows files when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-files')).toBeTruthy()
    expect(screen.getByText('Files Changed (3)')).toBeTruthy()
  })

  it('shows file paths', () => {
    renderPanel()
    expect(screen.getByText('src-tauri/src/task_scanner/mod.rs')).toBeTruthy()
    expect(screen.getByText('src/lib/TaskBoard.svelte')).toBeTruthy()
  })

  it('hides files section when empty', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-files')).toBeNull()
  })
})

describe('Dependencies section', () => {
  it('shows dependencies when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-dependencies')).toBeTruthy()
    expect(screen.getByText(/Blocked by/)).toBeTruthy()
    expect(screen.getByText(/Blocks/)).toBeTruthy()
  })

  it('hides dependencies for task without them', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-dependencies')).toBeNull()
  })
})

describe('Owner section', () => {
  it('shows owner when present', () => {
    renderPanel()
    expect(screen.getByTestId('detail-owner')).toBeTruthy()
    expect(screen.getByText('researcher')).toBeTruthy()
  })

  it('hides owner for task without one', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    expect(screen.queryByTestId('detail-owner')).toBeNull()
  })
})

describe('Sparse task rendering', () => {
  it('renders minimal panel for sparse Gemini task', () => {
    renderPanel({ task: SPARSE_TASK, detail: SPARSE_DETAIL })
    // Header always shows
    expect(screen.getByText('Write unit tests')).toBeTruthy()
    expect(screen.getByText('Gemini')).toBeTruthy()
    expect(screen.getByText('Pending')).toBeTruthy()
    // All detail sections absent
    expect(screen.queryByTestId('detail-description')).toBeNull()
    expect(screen.queryByTestId('detail-session')).toBeNull()
    expect(screen.queryByTestId('detail-commits')).toBeNull()
    expect(screen.queryByTestId('detail-files')).toBeNull()
    expect(screen.queryByTestId('detail-dependencies')).toBeNull()
    expect(screen.queryByTestId('detail-owner')).toBeNull()
  })
})

describe('Section structure', () => {
  it('wraps sections in a divide-y container for keyline dividers', () => {
    renderPanel()
    const sections = screen.getByTestId('detail-sections')
    expect(sections.className).toContain('divide-y')
  })

  it('has no section container during loading', () => {
    renderPanel({ detail: null })
    expect(screen.queryByTestId('detail-sections')).toBeNull()
  })
})

describe('Dark mode', () => {
  it('uses dark panel background in dark mode', () => {
    renderPanel({ dark: true })
    const panel = screen.getByTestId('task-detail-panel')
    expect(panel.className).toContain('bg-zinc-950')
  })

  it('uses light panel background in light mode', () => {
    renderPanel({ dark: false })
    const panel = screen.getByTestId('task-detail-panel')
    expect(panel.className).toContain('bg-white')
  })
})
