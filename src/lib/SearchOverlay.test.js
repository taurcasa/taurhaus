import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

function createDeferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

vi.mock('./ipc.js', () => ({
  search: vi.fn(),
}))

const { search } = await import('./ipc.js')
import SearchOverlay from './SearchOverlay.svelte'

describe('SearchOverlay', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('ignores stale async search results from older queries', async () => {
    const oldSearch = createDeferred()
    const newSearch = createDeferred()

    search.mockImplementation((query) => {
      if (query === 'old') return oldSearch.promise
      if (query === 'new') return newSearch.promise
      return Promise.resolve([])
    })

    render(SearchOverlay, {
      props: {
        open: true,
      },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'old' } })
    await vi.advanceTimersByTimeAsync(150)

    await fireEvent.input(input, { target: { value: 'new' } })
    await vi.advanceTimersByTimeAsync(150)

    newSearch.resolve([
      {
        entity_type: 'document',
        project_id: 'project-1',
        file_path: 'docs/new.md',
        title: 'New Result',
        snippet: 'new snippet',
      },
    ])
    await waitFor(() => {
      expect(screen.getByText('New Result')).toBeInTheDocument()
    })

    oldSearch.resolve([
      {
        entity_type: 'document',
        project_id: 'project-1',
        file_path: 'docs/old.md',
        title: 'Old Result',
        snippet: 'old snippet',
      },
    ])
    await vi.advanceTimersByTimeAsync(0)

    await waitFor(() => {
      expect(screen.getByText('New Result')).toBeInTheDocument()
    })
    expect(screen.queryByText('Old Result')).not.toBeInTheDocument()
  })

  it('shows empty prompt initially and no panel when closed', () => {
    const { rerender } = render(SearchOverlay, {
      props: { open: true },
    })

    expect(screen.getByText('Type to search across all projects')).toBeInTheDocument()

    rerender({ open: false })
    expect(screen.queryByTestId('search-overlay')).not.toBeInTheDocument()
  })

  it('shows no-results state when search resolves empty', async () => {
    search.mockResolvedValue([])

    render(SearchOverlay, {
      props: { open: true },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'nothing' } })
    await vi.advanceTimersByTimeAsync(150)

    await waitFor(() => {
      expect(screen.getByText('No matches found')).toBeInTheDocument()
    })
  })

  it('handles search rejection by showing no-results state', async () => {
    search.mockRejectedValue(new Error('boom'))

    render(SearchOverlay, {
      props: { open: true },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'broken' } })
    await vi.advanceTimersByTimeAsync(150)

    await waitFor(() => {
      expect(screen.getByText('No matches found')).toBeInTheDocument()
    })
  })

  it('clears query/results via clear button when not loading', async () => {
    search.mockResolvedValue([
      {
        entity_type: 'document',
        project_id: 'project-1',
        file_path: 'docs/readme.md',
        title: 'Readme',
        snippet: 'intro',
      },
    ])

    render(SearchOverlay, {
      props: { open: true },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'read' } })
    await vi.advanceTimersByTimeAsync(150)

    await waitFor(() => {
      expect(screen.getByText('Readme')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByRole('button', { name: 'Clear search' }))

    expect(screen.getByTestId('search-input')).toHaveValue('')
    expect(screen.getByText('Type to search across all projects')).toBeInTheDocument()
  })

  it('supports keyboard navigation and enter-to-open for document results', async () => {
    search.mockResolvedValue([
      {
        entity_type: 'document',
        project_id: 'project-1',
        file_path: 'src/main.rs',
        title: 'main.rs',
        snippet: 'fn main',
      },
      {
        entity_type: 'session',
        project_id: 'project-1',
        file_path: null,
        title: 'Session 12',
        snippet: 'summary',
      },
      {
        entity_type: 'commit',
        project_id: 'project-1',
        file_path: null,
        title: 'Fix crash',
        snippet: 'abc123',
      },
    ])
    const onNavigate = vi.fn()

    render(SearchOverlay, {
      props: { open: true, onNavigate },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'm' } })
    await vi.advanceTimersByTimeAsync(150)
    await waitFor(() => {
      expect(screen.getByText('main.rs')).toBeInTheDocument()
      expect(screen.getByText('Session 12')).toBeInTheDocument()
      expect(screen.getByText('Fix crash')).toBeInTheDocument()
    })

    await fireEvent.keyDown(input, { key: 'ArrowDown' }) // document
    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(onNavigate).toHaveBeenLastCalledWith({
      tab: 'files',
      filePath: 'src/main.rs',
      projectId: 'project-1',
    })
  })

  it('keeps selection/highlight behavior for grouped results when navigating by keyboard', async () => {
    search.mockResolvedValue([
      {
        entity_type: 'commit',
        project_id: 'project-3',
        file_path: null,
        title: 'Commit Match',
        snippet: 'commit snippet',
      },
      {
        entity_type: 'session',
        project_id: 'project-2',
        file_path: null,
        title: 'Session Match',
        snippet: 'session snippet',
      },
      {
        entity_type: 'document',
        project_id: 'project-1',
        file_path: 'src/main.rs',
        title: 'main.rs',
        snippet: 'fn main',
      },
    ])
    const onNavigate = vi.fn()

    render(SearchOverlay, {
      props: { open: true, onNavigate },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'match' } })
    await vi.advanceTimersByTimeAsync(150)
    await waitFor(() => {
      expect(screen.getByText('main.rs')).toBeInTheDocument()
      expect(screen.getByText('Session Match')).toBeInTheDocument()
      expect(screen.getByText('Commit Match')).toBeInTheDocument()
    })

    const rows = screen.getAllByTestId('search-result')

    await fireEvent.keyDown(input, { key: 'ArrowDown' })
    await fireEvent.keyDown(input, { key: 'ArrowDown' })

    expect(rows[1].className).toContain('bg-zinc-100')
    expect(rows[0].className).not.toContain('bg-zinc-100')

    await fireEvent.keyDown(input, { key: 'Enter' })
    expect(onNavigate).toHaveBeenLastCalledWith({
      tab: 'overview',
      section: 'session',
      projectId: 'project-2',
    })
  })

  it('maps session and commit results to overview navigation targets', async () => {
    search.mockResolvedValue([
      {
        entity_type: 'session',
        project_id: 'project-2',
        file_path: null,
        title: 'Session Match',
        snippet: 'session snippet',
      },
      {
        entity_type: 'commit',
        project_id: 'project-3',
        file_path: null,
        title: 'Commit Match',
        snippet: 'commit snippet',
      },
    ])
    const onNavigate = vi.fn()

    const { rerender } = render(SearchOverlay, {
      props: { open: true, onNavigate },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'match' } })
    await vi.advanceTimersByTimeAsync(150)
    await waitFor(() => expect(screen.getByText('Session Match')).toBeInTheDocument())
    await fireEvent.click(screen.getByText('Session Match'))
    expect(onNavigate).toHaveBeenLastCalledWith({
      tab: 'overview',
      section: 'session',
      projectId: 'project-2',
    })

    await rerender({ open: true, onNavigate })
    const reopenedInput = screen.getByTestId('search-input')
    await fireEvent.input(reopenedInput, { target: { value: 'match' } })
    await vi.advanceTimersByTimeAsync(150)
    await waitFor(() => expect(screen.getByText('Commit Match')).toBeInTheDocument())
    await fireEvent.click(screen.getByText('Commit Match'))
    expect(onNavigate).toHaveBeenLastCalledWith({
      tab: 'overview',
      section: 'commits',
      projectId: 'project-3',
    })
  })

  it('closes on escape and ignores backdrop inner clicks', async () => {
    render(SearchOverlay, {
      props: { open: true },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.keyDown(input, { key: 'Escape' })
    expect(screen.queryByTestId('search-overlay')).not.toBeInTheDocument()

    render(SearchOverlay, {
      props: { open: true },
    })
    const overlay = screen.getByTestId('search-overlay')
    const panel = overlay.querySelector('div.w-full')
    expect(panel).toBeTruthy()

    await fireEvent.click(panel)
    expect(screen.getByTestId('search-overlay')).toBeInTheDocument()
    await fireEvent.click(screen.getByRole('button', { name: 'Close search overlay' }))
    expect(screen.queryByTestId('search-overlay')).not.toBeInTheDocument()
  })

  it('exposes dialog semantics, traps focus, and restores prior focus on close', async () => {
    search.mockResolvedValue([])

    const trigger = document.createElement('button')
    trigger.textContent = 'Open Search'
    document.body.appendChild(trigger)
    trigger.focus()

    render(SearchOverlay, {
      props: { open: true },
    })

    await vi.advanceTimersByTimeAsync(16)

    const dialog = screen.getByRole('dialog', { name: 'Search across all projects' })
    expect(dialog).toHaveAttribute('aria-modal', 'true')

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'focus' } })
    await vi.advanceTimersByTimeAsync(150)

    const clearButton = screen.getByRole('button', { name: 'Clear search' })
    expect(input).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab', shiftKey: true })
    expect(clearButton).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Tab' })
    expect(input).toHaveFocus()

    await fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.queryByTestId('search-overlay')).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()

    trigger.remove()
  })
})
