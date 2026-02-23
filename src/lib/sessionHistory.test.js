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
}))

const { getArchivedSessions } = await import('./ipc.js')

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
})
