/**
 * SessionHistory component tests.
 *
 * Tests the accordion session group rendering, expand/collapse behavior,
 * task and commit list rendering, and edge states.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'

// Mock markdown rendering (MarkdownRenderer depends on shiki/WASM)
vi.mock('./markdown.js', () => ({
  renderMarkdown: vi.fn((source) => Promise.resolve(
    source ? `<p>${source}</p>` : ''
  )),
}))

// Mock IPC
vi.mock('./ipc.js', () => ({
  getArchivedSessions: vi.fn(),
  getCommitsInRange: vi.fn(),
}))

const { getArchivedSessions, getCommitsInRange } = await import('./ipc.js')

/** Build a mock archived session with defaults. */
function makeSession(overrides = {}) {
  return {
    session_id: 'sess-aaa',
    started_at: '2026-02-20T10:00:00Z',
    ended_at: '2026-02-20T12:15:00Z',
    duration_ms: 8100000,
    tasks: [
      { id: '1', subject: 'Task one', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null },
      { id: '2', subject: 'Task two', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null },
    ],
    commit_count: 5,
    file_count: 3,
    sources: ['claude'],
    ...overrides,
  }
}

import SessionHistory from './SessionHistory.svelte'

describe('SessionHistory component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Default: getCommitsInRange never resolves (keeps loading state)
    getCommitsInRange.mockReturnValue(new Promise(() => {}))
  })

  // --- Loading state ---

  it('shows loading skeleton before data arrives', () => {
    getArchivedSessions.mockReturnValue(new Promise(() => {}))

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    expect(screen.getByTestId('history-loading')).toBeTruthy()
  })

  // --- Empty state ---

  it('shows empty state when no archived sessions', async () => {
    getArchivedSessions.mockResolvedValue({ sessions: [], errors: [] })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('history-empty')).toBeTruthy()
    })
  })

  // --- Session headers ---

  it('renders session headers with date and duration', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const header = screen.getByTestId('session-header')
      expect(header).toBeTruthy()
      // Duration should be formatted
      expect(header.textContent).toContain('2h 15m')
    })
  })

  it('renders task and commit counts in header', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ commit_count: 12 })],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const header = screen.getByTestId('session-header')
      expect(header.textContent).toContain('2 tasks')
      expect(header.textContent).toContain('12 commits')
    })
  })

  it('renders multiple session headers', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [
        makeSession({ session_id: 'sess-1' }),
        makeSession({ session_id: 'sess-2', duration_ms: 3600000 }),
      ],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('session-header')).toHaveLength(2)
    })
  })

  // --- Expand/collapse ---

  it('sessions start collapsed (no task list visible)', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })
    // Expanded content should not be visible (grid-template-rows: 0fr)
    expect(screen.queryByTestId('session-detail')).toBeNull()
  })

  it('clicking header expands to show session detail', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-detail')).toBeTruthy()
    })
  })

  it('clicking header again collapses the session', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    // Expand
    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-detail')).toBeTruthy()
    })

    // Collapse
    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.queryByTestId('session-detail')).toBeNull()
    })
  })

  // --- Task list in expanded session ---

  it('shows tasks in expanded session', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByText('Task one')).toBeTruthy()
      expect(screen.getByText('Task two')).toBeTruthy()
    })
  })

  // --- Commit count + file count ---

  it('shows file count in expanded session', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ file_count: 8 })],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByText('8 files changed')).toBeTruthy()
    })
  })

  // --- Source tool icons ---

  it('renders source tool icons in header', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ sources: ['claude', 'gemini'] })],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByLabelText('Claude')).toBeTruthy()
      expect(screen.getByLabelText('Gemini')).toBeTruthy()
    })
  })

  // --- Error state ---

  it('shows error indicator when errors present', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: ['Could not resolve session time range for sess-xyz'],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('history-errors')).toBeTruthy()
    })
  })

  // --- Dark mode ---

  it('applies dark mode tokens', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: true } })
    await waitFor(() => {
      const header = screen.getByTestId('session-header')
      expect(header.className).toContain('text-zinc-100')
    })
  })

  it('applies light mode tokens', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const header = screen.getByTestId('session-header')
      expect(header.className).toContain('text-zinc-900')
    })
  })

  // --- Lazy-loaded commits ---

  it('shows loading skeleton for commits when expanding', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-commits-loading')).toBeTruthy()
    })
  })

  it('shows commits after lazy loading', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })
    getCommitsInRange.mockResolvedValue({
      commits: [
        { hash: 'aaa11111', message: 'First commit', author: 'Dev', date: '1h' },
        { hash: 'bbb22222', message: 'Second commit', author: 'Dev', date: '2h' },
      ],
      files: ['src/foo.js', 'src/bar.js'],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-commits')).toBeTruthy()
      expect(screen.getAllByTestId('session-commit')).toHaveLength(2)
    })
  })

  it('shows files after lazy loading', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })
    getCommitsInRange.mockResolvedValue({
      commits: [],
      files: ['src/a.js', 'src/b.js', 'src/c.js'],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-files')).toBeTruthy()
      expect(screen.getAllByTestId('session-file')).toHaveLength(3)
    })
  })

  // --- Navigation callbacks ---

  it('clicking commit calls onNavigateToCommit', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })
    getCommitsInRange.mockResolvedValue({
      commits: [{ hash: 'abc12345', message: 'Test commit', author: 'Dev', date: '1h' }],
      files: [],
    })

    const onNavigateToCommit = vi.fn()
    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false, onNavigateToCommit } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-commit')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-commit'))
    expect(onNavigateToCommit).toHaveBeenCalledWith('abc12345')
  })

  it('clicking file calls onNavigateToFile', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })
    getCommitsInRange.mockResolvedValue({
      commits: [],
      files: ['src/target.js'],
    })

    const onNavigateToFile = vi.fn()
    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false, onNavigateToFile } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('session-file')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-file'))
    expect(onNavigateToFile).toHaveBeenCalledWith('src/target.js')
  })

  it('shows "View in Git" button in expanded session', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('view-in-git')).toBeTruthy()
    })
  })

  it('clicking "View in Git" calls onNavigateToCommitRange', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    const onNavigateToCommitRange = vi.fn()
    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false, onNavigateToCommitRange } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      expect(screen.getByTestId('view-in-git')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('view-in-git'))
    expect(onNavigateToCommitRange).toHaveBeenCalledWith('2026-02-20T10:00:00Z', '2026-02-20T12:15:00Z')
  })
})
