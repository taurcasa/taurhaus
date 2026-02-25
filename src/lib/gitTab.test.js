/**
 * GitTab component tests.
 *
 * Tests commit list rendering, commit detail view, file click navigation,
 * range filter display, and cross-tab navigation handling.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock IntersectionObserver for JSDOM
globalThis.IntersectionObserver = class {
  constructor() {}
  observe() {}
  unobserve() {}
  disconnect() {}
}

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
  return Array.from({ length: n }, (_, i) => ({
    hash: `abc${String(i).padStart(5, '0')}`,
    message: `Commit message ${i + 1}`,
    body: opts.body ?? null,
    author: 'Developer',
    date: `${i + 1}h`,
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

import GitTab from './GitTab.svelte'

describe('GitTab component', () => {
  beforeEach(() => {
    vi.clearAllMocks()
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
      expect(screen.getByTestId('git-empty').textContent).toContain('No commits found')
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
      const row = screen.getByTestId('commit-row')
      expect(row.textContent).toContain('1h')
      expect(row.textContent).toContain('Commit message 1')
      expect(row.textContent).toContain('abc00000')
      expect(row.textContent).toContain('Developer')
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
      expect(getCommitFiles).toHaveBeenCalledWith('/test', 'abc00001')
    })
    expect(onClearNavTarget).toHaveBeenCalled()
  })

  // --- Dark mode ---

  it('applies dark mode classes', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: true } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      // In dark mode, unselected rows have dark text colors
      expect(row.className).toContain('text-zinc-400')
    })
  })

  it('applies light mode classes', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.className).toContain('text-zinc-600')
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
})
