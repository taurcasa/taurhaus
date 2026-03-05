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
})
