/**
 * GitTab component tests.
 *
 * Tests commit list rendering, commit detail view, file click navigation,
 * range filter display, and cross-tab navigation handling.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

// Mock IPC
vi.mock('./ipc.js', () => ({
  getAllCommits: vi.fn(),
  getCommitFiles: vi.fn(),
  getCommitsInRange: vi.fn(),
}))

const { getAllCommits, getCommitFiles, getCommitsInRange } = await import('./ipc.js')

/** Build mock commits. */
function makeCommits(n = 3) {
  return Array.from({ length: n }, (_, i) => ({
    hash: `abc${String(i).padStart(5, '0')}`,
    message: `Commit message ${i + 1}`,
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

  it('renders commit hash and message in row', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))

    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false } })
    await waitFor(() => {
      const row = screen.getByTestId('commit-row')
      expect(row.textContent).toContain('abc00000')
      expect(row.textContent).toContain('Commit message 1')
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

  // --- File click navigation ---

  it('clicking a file calls onNavigateToFile', async () => {
    getAllCommits.mockResolvedValue(makeCommits(1))
    getCommitFiles.mockResolvedValue([{ path: 'src/foo.js', status: 'modified' }])

    const onNavigateToFile = vi.fn()
    const { fireEvent } = await import('@testing-library/svelte')
    render(GitTab, { props: { projectPath: '/test', projectId: 'p1', dark: false, onNavigateToFile } })
    await waitFor(() => {
      expect(screen.getByTestId('commit-row')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-row'))
    await waitFor(() => {
      expect(screen.getByTestId('commit-file')).toBeTruthy()
    })

    await fireEvent.click(screen.getByTestId('commit-file'))
    expect(onNavigateToFile).toHaveBeenCalledWith('src/foo.js')
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
        gitNavTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
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
        gitNavTarget: { type: 'range', after: '2026-02-20T10:00:00Z', before: '2026-02-20T12:00:00Z' },
        onClearNavTarget: vi.fn(),
      },
    })
    await waitFor(() => {
      expect(screen.getByTestId('git-empty').textContent).toContain('No commits in this range')
    })
  })

  // --- Cross-tab navigation: commit ---

  it('gitNavTarget type=commit auto-selects the commit', async () => {
    const commits = makeCommits(3)
    getAllCommits.mockResolvedValue(commits)
    getCommitFiles.mockResolvedValue(makeFiles())

    const onClearNavTarget = vi.fn()
    render(GitTab, {
      props: {
        projectPath: '/test',
        projectId: 'p1',
        dark: false,
        gitNavTarget: { type: 'commit', hash: 'abc00001' },
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
