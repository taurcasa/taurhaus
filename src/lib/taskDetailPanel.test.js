/**
 * TaskDetailPanel component tests.
 *
 * Tests progressive disclosure (sections appear/disappear based on data),
 * loading state, sparse task rendering, and close behavior.
 */

import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte'

// Mock markdown rendering (MarkdownRenderer depends on shiki which needs WASM)
vi.mock('./markdown.js', () => ({
  renderMarkdown: vi.fn((source) => Promise.resolve(
    source ? `<p>${source}</p>` : ''
  )),
}))

import TaskDetailPanel from './TaskDetailPanel.svelte'

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/** A full task with all fields populated. */
const FULL_TASK = {
  id: '1',
  source_key: 'sess-abc',
  subject: 'Add task scanner backend',
  description: 'Parse tasks from all three CLI tools',
  active_form: 'Adding task scanner',
  status: 'in_progress',
  source: 'claude',
  blocks: ['2', '3'],
  blocked_by: ['0'],
  owner: 'researcher',
  session_id: 'abc-123-def',
  state_changed_at: null,
  updated_at: '2026-02-22T04:30:00.000Z',
  archived_at: null,
  last_status: null,
  archived_reason: null,
}

/** A sparse task (Gemini TODO item — only subject + status). */
const SPARSE_TASK = {
  id: 'todo-5',
  source_key: 'gemini-todo',
  subject: 'Write unit tests',
  description: null,
  active_form: null,
  status: 'pending',
  source: 'gemini',
  blocks: [],
  blocked_by: [],
  owner: null,
  session_id: null,
  state_changed_at: null,
  updated_at: null,
  archived_at: null,
  last_status: null,
  archived_reason: null,
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

/** Related tasks for dependency resolution. */
const RELATED_TASKS = [
  FULL_TASK,
  { id: '0', subject: 'Set up project scaffold', status: 'completed', source: 'claude', blocks: ['1'], blocked_by: [], owner: null },
  { id: '2', subject: 'Build TaskBoard UI', status: 'pending', source: 'claude', blocks: [], blocked_by: ['1'], owner: null },
  { id: '3', subject: 'Write integration tests', status: 'pending', source: 'claude', blocks: [], blocked_by: ['1'], owner: null },
  SPARSE_TASK,
]

function renderPanel(props = {}) {
  return render(TaskDetailPanel, {
    task: FULL_TASK,
    detail: FULL_DETAIL,
    dark: true,
    allTasks: RELATED_TASKS,
    onClose: vi.fn(),
    onNavigateTask: vi.fn(),
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
  it('shows description via MarkdownRenderer when present', async () => {
    renderPanel()
    expect(screen.getByTestId('detail-description')).toBeTruthy()
    // MarkdownRenderer renders asynchronously
    await waitFor(() => {
      expect(screen.getByText('Parse tasks from all three CLI tools')).toBeTruthy()
    })
  })

  it('renders description through markdown pipeline', async () => {
    renderPanel()
    await waitFor(() => {
      expect(screen.getByTestId('markdown-content')).toBeTruthy()
    })
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

describe('Archive context section', () => {
  it('shows archive context for archived task metadata', () => {
    const archivedTask = {
      ...FULL_TASK,
      archived_at: new Date(Date.now() - 3600000).toISOString(),
      archived_reason: 'completed_and_removed',
      last_status: 'completed',
    }
    renderPanel({
      task: archivedTask,
      detail: { ...FULL_DETAIL, task: archivedTask },
    })

    expect(screen.getByTestId('detail-archive-context')).toBeTruthy()
    expect(screen.getByText(/source removed/)).toBeTruthy()
    expect(screen.getByText(/Last status:/)).toBeTruthy()
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

  it('renders commit hashes as styled pills', () => {
    renderPanel()
    const pills = screen.getAllByTestId('commit-hash')
    expect(pills).toHaveLength(2)
    expect(pills[0].tagName).toBe('CODE')
    expect(pills[0].className).toContain('font-mono')
    expect(pills[0].className).toContain('rounded')
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

  it('splits file paths into directory and filename', () => {
    renderPanel()
    // Directory portions
    const dirs = screen.getAllByTestId('file-dir')
    expect(dirs.length).toBeGreaterThan(0)
    expect(dirs[0].textContent).toBe('src-tauri/src/task_scanner/')
    // Filename portions
    const names = screen.getAllByTestId('file-name')
    expect(names.length).toBe(3)
    expect(names[0].textContent).toBe('mod.rs')
  })

  it('renders filename at higher contrast than directory', () => {
    renderPanel()
    const dir = screen.getAllByTestId('file-dir')[0]
    const name = screen.getAllByTestId('file-name')[0]
    // Dir uses muted color, name uses secondary (higher contrast)
    expect(dir.className).toMatch(/text-zinc-[56]00/)
    expect(name.className).toMatch(/text-zinc-[36]00/)
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

  it('renders dependency chips with resolved task subjects', () => {
    renderPanel()
    const chips = screen.getAllByTestId('dep-chip')
    // FULL_TASK has blocks: ['2', '3'] and blocked_by: ['0'] = 3 chips total
    expect(chips).toHaveLength(3)
    expect(chips[0].textContent).toBe('#0 · Set up project scaffold')
    expect(chips[1].textContent).toBe('#2 · Build TaskBoard UI')
    expect(chips[2].textContent).toBe('#3 · Write integration tests')
  })

  it('renders resolved chips as clickable buttons', () => {
    renderPanel()
    const chips = screen.getAllByTestId('dep-chip')
    expect(chips[0].tagName).toBe('BUTTON')
    expect(chips[0].className).toContain('cursor-pointer')
  })

  it('calls onNavigateTask when dependency chip is clicked', async () => {
    const onNavigateTask = vi.fn()
    renderPanel({ onNavigateTask })
    const chip = screen.getAllByTestId('dep-chip')[0]
    await fireEvent.click(chip)
    expect(onNavigateTask).toHaveBeenCalledOnce()
    expect(onNavigateTask).toHaveBeenCalledWith(
      expect.objectContaining({ id: '0', subject: 'Set up project scaffold' })
    )
  })

  it('renders unresolved IDs as inert spans', () => {
    // Pass empty allTasks so nothing resolves
    renderPanel({ allTasks: [] })
    const chips = screen.getAllByTestId('dep-chip')
    expect(chips[0].tagName).toBe('SPAN')
    expect(chips[0].textContent).toBe('#0')
    expect(chips[0].className).toContain('opacity-60')
  })

  it('styles dependency chips with background and rounded corners', () => {
    renderPanel()
    const chip = screen.getAllByTestId('dep-chip')[0]
    expect(chip.className).toContain('font-mono')
    expect(chip.className).toContain('rounded')
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
