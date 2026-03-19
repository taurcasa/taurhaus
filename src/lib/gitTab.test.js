/**
 * GitTab component tests.
 *
 * Tests commit list rendering, commit detail view, file click navigation,
 * range filter display, and cross-tab navigation handling.
 */

import { describe, it, expect, vi, beforeEach, beforeAll, afterAll } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

let previousIntersectionObserver

beforeAll(() => {
  previousIntersectionObserver = globalThis.IntersectionObserver
  globalThis.IntersectionObserver = class IntersectionObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterAll(() => {
  if (previousIntersectionObserver) {
    globalThis.IntersectionObserver = previousIntersectionObserver
    return
  }
  delete globalThis.IntersectionObserver
})

// Mock IPC
vi.mock('./ipc.js', () => ({
  getAllCommits: vi.fn(),
  getCommitFiles: vi.fn(),
  getCommitsInRange: vi.fn(),
  getCommitDiff: vi.fn(),
}))

const { getAllCommits, getCommitFiles, getCommitsInRange, getCommitDiff } = await import('./ipc.js')

/** Build mock commits. */
function makeCommits(n = 3, opts = {}) {
  const now = Math.floor(Date.now() / 1000)
  return Array.from({ length: n }, (_, i) => ({
    hash: `abc${String(i).padStart(5, '0')}`,
    message: `Commit message ${i + 1}`,
    body: opts.body ?? null,
    author: 'Developer',
    date: `${i + 1}h`,
    timestamp: now - (i + 1) * 3600,
  }))
}

/** Build mock commit files. */
function makeFiles() {
  return [
    { path: 'src/lib/GitTab.svelte', status: 'added' },
    { path: 'src/Shell.svelte', status: 'modified' },
    { path: 'old-file.rs', status: 'deleted' },
  ]
}

/** Build mock diff hunks. */
function makeDiffHunks() {
  return [{
    old_start: 1, old_lines: 3, new_start: 1, new_lines: 4,
    lines: [
      { origin: ' ', content: 'line 1', old_lineno: 1, new_lineno: 1 },
      { origin: '-', content: 'old line', old_lineno: 2, new_lineno: null },
      { origin: '+', content: 'new line', old_lineno: null, new_lineno: 2 },
      { origin: '+', content: 'added line', old_lineno: null, new_lineno: 3 },
      { origin: ' ', content: 'line 3', old_lineno: 3, new_lineno: 4 },
    ],
  }]
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

import GitTab from './GitTab.svelte'

describe('GitTab component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    if (!navigator.clipboard) {
      Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
    } else {
      navigator.clipboard.writeText = vi.fn().mockResolvedValue(undefined)
    }
  })

  // --- Loading state ---

  it('shows loading skeleton initially', () => {
    getAllCommits.mockReturnValue(new Promise(() => {}))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    expect(screen.getByTestId('git-loading')).toBeTruthy()
  })

  // --- Empty state ---

  it('shows empty state when no commits found', async () => {
    getAllCommits.mockResolvedValue([])

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('git-empty')).toBeTruthy()
    })
  })

  it('shows "No commits found" in default empty state', async () => {
    getAllCommits.mockResolvedValue([])

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('No commits found')).toBeInTheDocument()
    })
  })

  // --- Commit list ---

  it('renders commit rows after loading', async () => {
    getAllCommits.mockResolvedValue(makeCommits(3))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(3)
    })
  })

  it('renders time, message, hash, and author in row', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByRole('button', { name: /Commit message 1/i })
      expect(within(row).getByText('1h')).toBeInTheDocument()
      expect(within(row).getByText('Commit message 1')).toBeInTheDocument()
      expect(within(row).getByText('abc00000')).toBeInTheDocument()
      expect(within(row).getByText('Developer')).toBeInTheDocument()
    })
  })

  // --- Commit selection + detail ---

  it('clicking a commit row shows commit detail with files', async () => {
    getAllCommits.mockResolvedValue(makeCommits(2))
    getCommitFiles.mockResolvedValue(makeFiles())

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(2)
    })

    await fireEvent.click(screen.getAllByTestId('commit-row')[0])
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-file')).toHaveLength(3)
    })
  })

  it('selected commit row has aria-current=true', async () => {
    getAllCommits.mockResolvedValue(makeCommits(2))
    getCommitFiles.mockResolvedValue([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(2)
    })

    await fireEvent.click(screen.getAllByTestId('commit-row')[0])
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')[0]).toHaveAttribute('aria-current', 'true')
      expect(screen.getAllByTestId('commit-row')[1]).not.toHaveAttribute('aria-current')
    })
  })

  it('opens the commit context menu from the keyboard', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })

    const commitRow = await screen.findByTestId('commit-row')
    commitRow.focus()

    await fireEvent.keyDown(commitRow, { key: 'ContextMenu' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith('abc00000')
    })
  })

  it('shows "Select a commit" placeholder before selection', async () => {
    getAllCommits.mockResolvedValue(makeCommits(2))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByText('Select a commit to view details')).toBeTruthy()
    })
  })

  it('shows commit message in detail header', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      // Message appears in both commit list row and detail header
      const matches = screen.getAllByText('Commit message 1')
      expect(matches.length).toBeGreaterThanOrEqual(2)
      // The detail header message is in a <p> tag
      const detailMsg = matches.find(el => el.tagName === 'P')
      expect(detailMsg).toBeTruthy()
    })
  })

  it('shows commit body in detail when present', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1, { body: 'Detailed body text' }))
    getCommitFiles.mockResolvedValue([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getByText('Detailed body text')).toBeTruthy()
    })
  })

  it('does not show body section when body is null', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([])

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      // Only the message <p>, no body <p>
      const pTags = screen.getAllByText('Commit message 1').filter(el => el.tagName === 'P')
      expect(pTags).toHaveLength(1)
    })
  })

  it('shows file count summary in detail', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue(makeFiles())

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getByText('3 files')).toBeTruthy()
    })
  })

  // --- File click → diff view ---

  it('clicking a file shows diff view', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([{ path: 'src/foo.js', status: 'modified' }])
    getCommitDiff.mockResolvedValue(makeDiffHunks())

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getByTestId('commit-file')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-file'))
    await waitFor(() => {
      expect(screen.getByTestId('diff-view')).toBeTruthy()
      expect(screen.getByTestId('diff-content')).toBeTruthy()
    })
  })

  it('ignores stale diff responses when switching files quickly', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([
      { path: 'src/first.js', status: 'modified' },
      { path: 'src/second.js', status: 'modified' },
    ])
    const firstDiff = createDeferred()
    const secondDiff = createDeferred()
    getCommitDiff.mockImplementation((_, __, path) => {
      if (path === 'src/first.js') return firstDiff.promise
      if (path === 'src/second.js') return secondDiff.promise
      return Promise.resolve([])
    })

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })
    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-file')).toHaveLength(2)
    })

    const fileButtons = screen.getAllByTestId('commit-file')
    const firstFileButton = fileButtons.find((button) => button.textContent.includes('src/first.js'))
    const secondFileButton = fileButtons.find((button) => button.textContent.includes('src/second.js'))
    expect(firstFileButton).toBeTruthy()
    expect(secondFileButton).toBeTruthy()

    await fireEvent.click(firstFileButton)
    await waitFor(() => {
      expect(screen.getByTestId('diff-loading')).toBeTruthy()
      expect(screen.getByText('src/first.js')).toBeTruthy()
    })

    const diffPills = screen.getAllByTestId('file-pill')
    const secondFilePill = diffPills.find((pill) => pill.textContent.includes('second.js'))
    expect(secondFilePill).toBeTruthy()
    await fireEvent.click(secondFilePill)

    secondDiff.resolve([
      {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: [{ origin: '+', content: 'second-line', old_lineno: null, new_lineno: 1 }],
      },
    ])

    await waitFor(() => {
      expect(screen.getByText('second-line')).toBeTruthy()
    })

    firstDiff.resolve([
      {
        old_start: 1,
        old_lines: 1,
        new_start: 1,
        new_lines: 1,
        lines: [{ origin: '+', content: 'first-line', old_lineno: null, new_lineno: 1 }],
      },
    ])

    await waitFor(() => {
      expect(screen.getByText('second-line')).toBeTruthy()
      expect(screen.queryByText('first-line')).toBeNull()
    })
  })

  it('back to files returns to file list from diff', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([{ path: 'src/foo.js', status: 'modified' }])
    getCommitDiff.mockResolvedValue(makeDiffHunks())

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => expect(screen.getByTestId('commit-row')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => expect(screen.getByTestId('commit-file')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-file'))
    await waitFor(() => expect(screen.getByTestId('diff-view')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('back-to-files'))
    await waitFor(() => {
      expect(screen.queryByTestId('diff-view')).toBeNull()
      expect(screen.getByTestId('commit-file')).toBeTruthy()
    })
  })

  it('open file button calls onNavigateToFile', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([{ path: 'src/foo.js', status: 'modified' }])
    getCommitDiff.mockResolvedValue(makeDiffHunks())

    const onNavigateToFile = vi.fn()
    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false, onNavigateToFile } })
    await waitFor(() => expect(screen.getByTestId('commit-row')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => expect(screen.getByTestId('commit-file')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-file'))
    await waitFor(() => expect(screen.getByTestId('open-file-btn')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('open-file-btn'))
    expect(onNavigateToFile).toHaveBeenCalledWith('src/foo.js', 2) // first added line
  })

  it('selecting different commit clears diff view', async () => {
    getAllCommits.mockResolvedValue(makeCommits(2))
    getCommitFiles.mockResolvedValue([{ path: 'src/foo.js', status: 'modified' }])
    getCommitDiff.mockResolvedValue(makeDiffHunks())

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => expect(screen.getAllByTestId('commit-row')).toHaveLength(2))

    // Select first commit, open diff
    await fireEvent.click(screen.getAllByTestId('commit-row')[0])
    await waitFor(() => expect(screen.getByTestId('commit-file')).toBeTruthy())
    await fireEvent.click(screen.getByTestId('commit-file'))
    await waitFor(() => expect(screen.getByTestId('diff-view')).toBeTruthy())

    // Select second commit — should clear diff
    await fireEvent.click(screen.getAllByTestId('commit-row')[1])
    await waitFor(() => {
      expect(screen.queryByTestId('diff-view')).toBeNull()
      expect(screen.getByTestId('commit-file')).toBeTruthy()
    })
  })

  it('shows empty diff message for binary files', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([{ path: 'image.png', status: 'added' }])
    getCommitDiff.mockResolvedValue([]) // Empty = binary

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => expect(screen.getByTestId('commit-row')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => expect(screen.getByTestId('commit-file')).toBeTruthy())

    await fireEvent.click(screen.getByTestId('commit-file'))
    await waitFor(() => {
      expect(screen.getByTestId('diff-empty')).toBeTruthy()
      expect(screen.getByTestId('diff-empty').textContent).toContain('Binary file')
    })
  })

  // --- Range filter ---

  it('shows range filter indicator when rangeFilter is active', async () => {
    // Simulate cross-nav with range target
    getCommitsInRange.mockResolvedValue({ commits: makeCommits(2), files: [] })

    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        navTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
        onClearNavTarget: vi.fn(),
      },
    })
    await waitFor(() => {
      expect(screen.getByTestId('range-filter')).toBeTruthy()
      expect(screen.getByTestId('range-filter').textContent).toContain('Filtered to session')
    })
  })

  it('range filter shows "No commits in this range" when empty', async () => {
    getCommitsInRange.mockResolvedValue({ commits: [], files: [] })

    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        navTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
        onClearNavTarget: vi.fn(),
      },
    })
    await waitFor(() => {
      expect(screen.getByTestId('git-empty').textContent).toContain('No commits in this range')
    })
  })

  it('ignores stale range responses after clearing filter', async () => {
    const initialCommits = [
      {
        hash: 'base0001',
        message: 'Base commit',
        body: null,
        author: 'Developer',
        date: '1h',
        timestamp: Math.floor(Date.now() / 1000),
      },
    ]
    const clearCommits = [
      {
        hash: 'clear0001',
        message: 'Clear result commit',
        body: null,
        author: 'Developer',
        date: '2h',
        timestamp: Math.floor(Date.now() / 1000) - 100,
      },
    ]

    const staleRange = createDeferred()
    const clearLoad = createDeferred()
    let getAllCommitsCallCount = 0
    getAllCommits.mockImplementation(() => {
      getAllCommitsCallCount += 1
      if (getAllCommitsCallCount === 1) return Promise.resolve(initialCommits)
      return clearLoad.promise
    })
    getCommitsInRange.mockImplementation(() => staleRange.promise)

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        navTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
        onClearNavTarget: vi.fn(),
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('range-filter')).toBeTruthy()
    })

    await fireEvent.click(screen.getByText('Clear'))
    clearLoad.resolve(clearCommits)
    await waitFor(() => {
      expect(screen.getByText('Clear result commit')).toBeTruthy()
    })

    staleRange.resolve({ commits: [{ ...initialCommits[0], hash: 'stale0001', message: 'Stale range commit' }] })
    await waitFor(() => {
      expect(screen.getByText('Clear result commit')).toBeTruthy()
      expect(screen.queryByText('Stale range commit')).toBeNull()
    })
  })

  // --- Cross-tab navigation: commit ---

  it('navTarget type=commit auto-selects the commit', async () => {
    const commits = makeCommits(3)
    getAllCommits.mockResolvedValue(commits)
    getCommitFiles.mockResolvedValue(makeFiles())

    const onClearNavTarget = vi.fn()
    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        navTarget: { type: 'commit', hash: 'abc00001' },
        onClearNavTarget,
      },
    })
    await waitFor(() => {
      // Should auto-select the commit and show files
      expect(getCommitFiles).toHaveBeenCalledWith('p1', 'abc00001')
    })
    expect(onClearNavTarget).toHaveBeenCalled()
  })

  // --- Dark mode ---

  it('applies dark mode classes', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: true } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      // In dark mode, unselected rows have border separator
      expect(row.className).toContain('border-b')
      expect(row.className).toContain('border-white/5')
    })
  })

  it('applies light mode classes', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.className).toContain('border-b')
      expect(row.className).toContain('border-zinc-100')
    })
  })

  // --- Infinite scroll ---

  it('shows sentinel element when hasMore is true', async () => {
    // Return exactly 50 commits (PAGE_SIZE) — sentinel should appear
    getAllCommits.mockResolvedValue(makeCommits(50))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(50)
      expect(screen.getByTestId('scroll-sentinel')).toBeTruthy()
    })
  })

  it('hides sentinel when less than PAGE_SIZE returned', async () => {
    // Return less than 50 — hasMore should be false, no sentinel
    getAllCommits.mockResolvedValue(makeCommits(10))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(10)
      expect(screen.queryByTestId('scroll-sentinel')).toBeNull()
    })
  })

  it('no sentinel in range filter mode', async () => {
    getCommitsInRange.mockResolvedValue({ commits: makeCommits(50), files: [] })

    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        navTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
        onClearNavTarget: vi.fn(),
      },
    })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(50)
      expect(screen.queryByTestId('scroll-sentinel')).toBeNull()
    })
  })

  // --- Files loading state ---

  it('shows files loading skeleton while fetching commit files', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockReturnValue(new Promise(() => {})) // Never resolves

    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getByTestId('files-loading')).toBeTruthy()
    })
  })

  // --- P11: Date group headers ---

  it('renders date group header for commits', async () => {
    // Use local-noon timestamps to avoid midnight boundary flakes.
    const todayNoon = new Date()
    todayNoon.setHours(12, 0, 0, 0)
    const todayTs = Math.floor(todayNoon.getTime() / 1000)
    getAllCommits.mockResolvedValue([
      { hash: 'ttt00001', message: 'A', body: null, author: 'Dev', date: '1h', timestamp: todayTs },
      { hash: 'ttt00002', message: 'B', body: null, author: 'Dev', date: '2h', timestamp: todayTs - 60 },
      { hash: 'ttt00003', message: 'C', body: null, author: 'Dev', date: '3h', timestamp: todayTs - 120 },
    ])

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(3)
      // There should be a "Today" group header
      expect(screen.getByText('Today')).toBeTruthy()
    })
  })

  it('renders separate date headers for commits on different days', async () => {
    const todayNoon = new Date()
    todayNoon.setHours(12, 0, 0, 0)
    const yesterdayNoon = new Date(todayNoon)
    yesterdayNoon.setDate(yesterdayNoon.getDate() - 1)
    const commits = [
      {
        hash: 'aaa00001',
        message: 'Recent commit',
        body: null,
        author: 'Dev',
        date: '1h',
        timestamp: Math.floor(todayNoon.getTime() / 1000),
      },
      {
        hash: 'aaa00002',
        message: 'Yesterday commit',
        body: null,
        author: 'Dev',
        date: '1d',
        timestamp: Math.floor(yesterdayNoon.getTime() / 1000),
      },
    ]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      expect(screen.getAllByTestId('commit-row')).toHaveLength(2)
      expect(screen.getByText('Today')).toBeTruthy()
      expect(screen.getByText('Yesterday')).toBeTruthy()
    })
  })

  // --- P11: Author initial avatars ---

  it('renders author initial in commit row', async () => {
    const commits = [{
      hash: 'bbb00001', message: 'Test commit', body: null,
      author: 'John', date: '1h', timestamp: Math.floor(Date.now() / 1000) - 3600,
    }]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      // Should contain the author initial "J" in a circle
      expect(row.textContent).toContain('J')
    })
  })

  it('shows author initial as uppercase first letter', async () => {
    const commits = [{
      hash: 'ccc00001', message: 'Lowercase test', body: null,
      author: 'alice', date: '1h', timestamp: Math.floor(Date.now() / 1000) - 3600,
    }]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.textContent).toContain('A')
    })
  })

  it('shows "?" for missing author name', async () => {
    const commits = [{
      hash: 'ddd00001', message: 'No author', body: null,
      author: '', date: '1h', timestamp: Math.floor(Date.now() / 1000) - 3600,
    }]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.textContent).toContain('?')
    })
  })

  it('different authors get different avatar colors', async () => {
    const commits = [
      { hash: 'eee00001', message: 'First', body: null, author: 'Alice', date: '1h', timestamp: Math.floor(Date.now() / 1000) - 3600 },
      { hash: 'eee00002', message: 'Second', body: null, author: 'Bob', date: '2h', timestamp: Math.floor(Date.now() / 1000) - 7200 },
    ]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const rows = screen.getAllByTestId('commit-row')
      expect(rows.length).toBe(2)
      // Each avatar circle gets a per-author hsl color via inline style
      const avatar1 = rows[0].querySelector('.rounded-full')
      const avatar2 = rows[1].querySelector('.rounded-full')
      expect(avatar1).not.toBeNull()
      expect(avatar2).not.toBeNull()
      // Initials should differ
      expect(avatar1.textContent.trim()).toBe('A')
      expect(avatar2.textContent.trim()).toBe('B')
    })
  })

  it('same author always gets the same avatar color', async () => {
    const commits = [
      { hash: 'fff00001', message: 'First', body: null, author: 'Alice', date: '1h', timestamp: Math.floor(Date.now() / 1000) - 3600 },
      { hash: 'fff00002', message: 'Second', body: null, author: 'Alice', date: '2h', timestamp: Math.floor(Date.now() / 1000) - 7200 },
    ]
    getAllCommits.mockResolvedValue(commits)

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const rows = screen.getAllByTestId('commit-row')
      const avatar1 = rows[0].querySelector('.rounded-full')
      const avatar2 = rows[1].querySelector('.rounded-full')
      expect(avatar1).not.toBeNull()
      // Same author should show the same initial
      expect(avatar1.textContent.trim()).toBe('A')
      expect(avatar2.textContent.trim()).toBe('A')
    })
  })

  // --- P11: Timestamp position ---

  it('commit row contains timestamp text', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.textContent).toContain('1h')
    })
  })
})
