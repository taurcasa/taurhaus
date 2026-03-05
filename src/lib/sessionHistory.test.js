/**
 * SessionHistory component tests.
 *
 * Tests the accordion session group rendering, expand/collapse behavior,
 * task and commit list rendering, and edge states.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor, within } from '@testing-library/svelte'

const { eventListenMock, emitProjectTasksChanged } = vi.hoisted(() => {
  let handler = null
  return {
    eventListenMock: vi.fn(async (_event, cb) => {
      handler = cb
      return () => {}
    }),
    emitProjectTasksChanged: (payload) => {
      if (handler) handler({ payload })
    },
  }
})

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
vi.mock('@tauri-apps/api/event', () => ({
  listen: eventListenMock,
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
      { id: '1', source_key: 'sess-aaa', subject: 'Task one', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null, archived_reason: null, archived_at: null, last_status: null },
      { id: '2', source_key: 'sess-aaa', subject: 'Task two', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null, archived_reason: null, archived_at: null, last_status: null },
    ],
    commit_count: 5,
    file_count: 3,
    sources: ['claude'],
    enrichment_warnings: [],
    ...overrides,
  }
}

import SessionHistory from './SessionHistory.svelte'

describe('SessionHistory component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    delete window.__TAURI_INTERNALS__
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
      const header = screen.getByRole('button', { name: /2h 15m/i })
      expect(within(header).getByText('2h 15m')).toBeTruthy()
    })
  })

  it('renders task and commit counts in header', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ commit_count: 12 })],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const header = screen.getByRole('button', { name: /12 commits/i })
      expect(within(header).getByText('2 tasks')).toBeTruthy()
      expect(within(header).getByText('12 commits')).toBeTruthy()
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

  // --- Last archived indicator ---

  it('shows last archived relative time when present', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ last_archived_at: new Date(Date.now() - 3600000).toISOString() })],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      const el = screen.getByTestId('last-archived')
      expect(el).toBeTruthy()
      expect(el.textContent).toContain('archived 1h ago')
    })
  })

  it('does not show last archived when field is missing', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession()],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })
    expect(screen.queryByTestId('last-archived')).toBeNull()
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

  it('shows enrichment warning badge when session has warning metadata', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({ enrichment_warnings: ['Could not resolve transcript range'] })],
      errors: [],
    })

    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-enrichment-warning')).toBeTruthy()
    })
  })

  it('renders archive reason/time chip on archived task rows', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [makeSession({
        tasks: [
          { id: '1', source_key: 'sess-aaa', subject: 'Archived task', status: 'completed', source: 'claude', blocks: [], blocked_by: [], owner: null, description: null, active_form: null, archived_reason: 'completed_and_removed', archived_at: new Date(Date.now() - 7200000).toISOString(), last_status: 'completed' },
        ],
      })],
      errors: [],
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(SessionHistory, { props: { projectPath: '/test', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('session-header')).toBeTruthy()
    })
    await fireEvent.click(screen.getByTestId('session-header'))
    await waitFor(() => {
      const chip = screen.getByTestId('history-archive-chip')
      expect(chip.textContent).toContain('Archived: source removed')
    })
  })

  it('re-fetches archived sessions on project-tasks-changed event while active', async () => {
    window.__TAURI_INTERNALS__ = {}
    getArchivedSessions
      .mockResolvedValueOnce({ sessions: [makeSession()], errors: [] })
      .mockResolvedValueOnce({ sessions: [makeSession({ session_id: 'sess-bbb' })], errors: [] })

    render(SessionHistory, { props: { projectPath: '/test', projectId: 'proj-1', isActive: true, dark: false } })
    await waitFor(() => {
      expect(getArchivedSessions).toHaveBeenCalledTimes(1)
    })
    await waitFor(() => {
      expect(eventListenMock).toHaveBeenCalled()
    })

    emitProjectTasksChanged({ project_id: 'proj-1' })
    await waitFor(() => {
      expect(getArchivedSessions).toHaveBeenCalledTimes(2)
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
