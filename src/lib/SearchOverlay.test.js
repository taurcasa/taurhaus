import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'
import '../app.css'

function createDeferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

const { eventListenMock, emitSearchIndexUpdated } = vi.hoisted(() => {
  /** @type {{ event: string, handler: (event: any) => void }[]} */
  let handlers = []
  return {
    eventListenMock: vi.fn(async (event, cb) => {
      handlers.push({ event, handler: cb })
      return () => {
        handlers = handlers.filter((entry) => entry.handler !== cb)
      }
    }),
    emitSearchIndexUpdated: (payload) => {
      handlers
        .filter((entry) => entry.event === 'search-index-updated')
        .forEach((entry) => entry.handler({ payload }))
    },
  }
})

vi.mock('./ipc.js', () => ({
  search: vi.fn(),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: eventListenMock,
}))

const { search } = await import('./ipc.js')
import SearchOverlay from './SearchOverlay.svelte'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

describe('SearchOverlay', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers()
    delete window.__TAURI_INTERNALS__
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

    const overlay = screen.getByTestId('search-overlay')
    expect(overlay.className).toContain('fixed')
    expect(overlay.className).not.toContain('relative')
    expect(screen.getByText('Type to search across all projects')).toBeInTheDocument()

    rerender({ open: false })
    expect(screen.queryByTestId('search-overlay')).not.toBeInTheDocument()
  })

  it('keeps the search overlay out of the shell frame flow', () => {
    const shellFrame = document.createElement('div')
    shellFrame.className = 'shell-frame'
    document.body.appendChild(shellFrame)

    const mainContent = document.createElement('div')
    mainContent.setAttribute('data-testid', 'shell-main-content')
    shellFrame.appendChild(mainContent)

    render(SearchOverlay, {
      target: shellFrame,
      props: { open: true },
    })

    // Regression: commit 188211f reintroduced `.shell-frame > * { position: relative }`,
    // which overrode Tailwind's `.fixed` on direct-child overlays and pushed both
    // SearchOverlay and other overlays into normal document flow.
    const overlay = screen.getByTestId('search-overlay')
    expect(overlay).toHaveAttribute('data-shell-overlay')
    expect(appCss).toContain('.shell-frame > :not([data-shell-overlay])')
    expect(appCss).not.toContain('.shell-frame > * {\n  position: relative;')
    expect(shellFrame.firstElementChild).toBe(mainContent)
    expect(shellFrame.lastElementChild).toBe(overlay)

    shellFrame.remove()
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

  it('makes sibling shell content inert while the overlay is open', async () => {
    const shellFrame = document.createElement('div')
    const shellContent = document.createElement('div')
    shellContent.setAttribute('data-testid', 'shell-content')
    shellFrame.appendChild(shellContent)
    document.body.appendChild(shellFrame)

    const { rerender } = render(SearchOverlay, {
      target: shellFrame,
      props: { open: true },
    })

    await waitFor(() => {
      expect(shellContent).toHaveAttribute('inert')
      expect(shellContent).toHaveAttribute('aria-hidden', 'true')
    })

    await rerender({ open: false })

    expect(shellContent).not.toHaveAttribute('inert')
    expect(shellContent).not.toHaveAttribute('aria-hidden')
    shellFrame.remove()
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

  it('adds visible keyboard focus styles to the search input and result rows', async () => {
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
    expect(input.className).toContain('focus:ring-1')
    expect(input.className).toContain('focus:ring-brand-500')

    await fireEvent.input(input, { target: { value: 'read' } })
    await vi.advanceTimersByTimeAsync(150)

    await waitFor(() => {
      expect(screen.getByText('Readme')).toBeInTheDocument()
    })

    const result = screen.getByTestId('search-result')
    expect(result.className).toContain('focus-visible:ring-1')
    expect(result.className).toContain('focus-visible:ring-brand-500')
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

  it('re-runs the active query when the backend reports a search index update', async () => {
    window.__TAURI_INTERNALS__ = {}
    search
      .mockResolvedValueOnce([
        {
          entity_type: 'document',
          project_id: 'project-1',
          file_path: 'docs/old.md',
          title: 'Old Result',
          snippet: 'old snippet',
        },
      ])
      .mockResolvedValueOnce([
        {
          entity_type: 'document',
          project_id: 'project-1',
          file_path: 'docs/new.md',
          title: 'Updated Result',
          snippet: 'new snippet',
        },
      ])

    render(SearchOverlay, {
      props: { open: true },
    })

    const input = screen.getByTestId('search-input')
    await fireEvent.input(input, { target: { value: 'docs' } })
    await vi.advanceTimersByTimeAsync(150)

    await waitFor(() => {
      expect(screen.getByText('Old Result')).toBeInTheDocument()
    })

    emitSearchIndexUpdated({ project_id: 'project-1', reason: 'file_changed' })

    await waitFor(() => {
      expect(search).toHaveBeenCalledTimes(2)
      expect(screen.getByText('Updated Result')).toBeInTheDocument()
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
