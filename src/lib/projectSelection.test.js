import { afterEach, describe, expect, it, vi } from 'vitest'

import { createProjectSelectionRequests, loadProjectSelectionData, withFallback } from './projectSelection.js'

afterEach(() => {
  vi.useRealTimers()
})

describe('projectSelection timeouts', () => {
  it('withFallback returns fallback when a section request times out', async () => {
    vi.useFakeTimers()

    const unresolved = new Promise(() => {})
    const resultPromise = withFallback('Session history', unresolved, [], 50)

    await vi.advanceTimersByTimeAsync(51)
    const result = await resultPromise

    expect(result.ok).toBe(false)
    expect(result.section).toBe('Session history')
    expect(result.value).toEqual([])
    expect(result.message.toLowerCase()).toContain('session history')
  })

  it('createProjectSelectionRequests resolves all sections with fallbacks when IPC hangs', async () => {
    vi.useFakeTimers()

    const never = () => new Promise(() => {})
    const requests = createProjectSelectionRequests('p1', {
      getProject: never,
      getRecentCommits: never,
      getLatestSession: never,
      listSessions: never,
      getReadme: never,
      getRelationships: never,
    })

    const resultsPromise = Promise.all([
      requests.detail,
      requests.commits,
      requests.latest,
      requests.sessionList,
      requests.readme,
      requests.rels,
    ])

    await vi.advanceTimersByTimeAsync(5001)
    const results = await resultsPromise

    expect(results.every((entry) => entry.ok === false)).toBe(true)
    expect(results[0].value).toBe(null)
    expect(results[1].value).toEqual([])
    expect(results[3].value).toEqual([])
    expect(results[4].value).toBe(null)
    expect(results[5].value).toEqual([])
  })

  it('coalesces rapid project switches so only the final IPC batch starts', async () => {
    vi.useFakeTimers()

    const createResolvedIpc = () => ({
      getProject: vi.fn((projectId) => Promise.resolve({ id: projectId })),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    })

    const ipc = createResolvedIpc()
    const first = loadProjectSelectionData('p1', ipc)
    const second = loadProjectSelectionData('p2', ipc)
    const third = loadProjectSelectionData('p3', ipc)

    expect(ipc.getProject).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(26)
    await Promise.all([first, second, third])

    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getProject).toHaveBeenCalledWith('p3')
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
  })

  it('still starts separate IPC batches for non-rapid project switches', async () => {
    vi.useFakeTimers()

    const ipc = {
      getProject: vi.fn((projectId) => Promise.resolve({ id: projectId })),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const first = loadProjectSelectionData('p1', ipc)
    await vi.advanceTimersByTimeAsync(26)
    await first

    const second = loadProjectSelectionData('p2', ipc)
    await vi.advanceTimersByTimeAsync(26)
    await second

    expect(ipc.getProject).toHaveBeenCalledTimes(2)
    expect(ipc.getProject).toHaveBeenNthCalledWith(1, 'p1')
    expect(ipc.getProject).toHaveBeenNthCalledWith(2, 'p2')
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(2)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(2)
    expect(ipc.listSessions).toHaveBeenCalledTimes(2)
    expect(ipc.getReadme).toHaveBeenCalledTimes(2)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(2)
  })

  it('starts the IPC batch immediately when debounce is overridden to zero', async () => {
    const ipc = {
      getProject: vi.fn((projectId) => Promise.resolve({ id: projectId })),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const result = await loadProjectSelectionData('p1', ipc, { debounceMs: 0 })

    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getProject).toHaveBeenCalledWith('p1')
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
    expect(result.detail.value).toEqual({ id: 'p1' })
  })
})
