import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  classifyProjectLoadResults,
  createProjectSelectionRequests,
  loadDeferredProjectSelectionData,
  loadProjectSelectionData,
  prefetchProjectSelectionData,
  resetProjectSelectionStateForTests,
  withFallback,
} from './projectSelection.js'

afterEach(() => {
  vi.useRealTimers()
  resetProjectSelectionStateForTests()
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

  it('marks transient daemon reconnect failures as retryable', async () => {
    const result = await withFallback(
      'Recent commits',
      Promise.reject(new Error('Daemon transport error: recent commits is unavailable for WSL UNC repositories without a connected daemon')),
      []
    )

    expect(result.ok).toBe(false)
    expect(result.retryableOnDaemonReconnect).toBe(true)
  })

  it('does not mark unrelated project-load failures as retryable', async () => {
    const result = await withFallback(
      'README',
      Promise.reject(new Error('README parsing failed')),
      null
    )

    expect(result.ok).toBe(false)
    expect(result.retryableOnDaemonReconnect).toBe(false)
  })

  it('defers retryable daemon-related issues while startup recovery is still pending', () => {
    const classified = classifyProjectLoadResults(
      [
        {
          ok: false,
          section: 'Recent commits',
          message: 'Daemon transport error: recent commits is unavailable for WSL UNC repositories without a connected daemon',
          retryableOnDaemonReconnect: true,
        },
        {
          ok: false,
          section: 'README',
          message: 'README parsing failed',
          retryableOnDaemonReconnect: false,
        },
      ],
      { deferRetryableIssues: true }
    )

    expect(classified.pendingRetry).toBe(true)
    expect(classified.visibleIssues).toEqual([
      {
        section: 'README',
        message: 'README parsing failed',
        retryableOnDaemonReconnect: false,
      },
    ])
  })

  it('surfaces retryable issues once daemon recovery is no longer pending', () => {
    const classified = classifyProjectLoadResults(
      [
        {
          ok: false,
          section: 'Recent commits',
          message: 'Daemon transport error: recent commits is unavailable for WSL UNC repositories without a connected daemon',
          retryableOnDaemonReconnect: true,
        },
      ],
      { deferRetryableIssues: false }
    )

    expect(classified.pendingRetry).toBe(false)
    expect(classified.visibleIssues).toEqual([
      {
        section: 'Recent commits',
        message: 'Daemon transport error: recent commits is unavailable for WSL UNC repositories without a connected daemon',
        retryableOnDaemonReconnect: true,
      },
    ])
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

  it('emits section timing with provider route and blocking flags', async () => {
    vi.useFakeTimers()

    const logger = {
      info: vi.fn(),
      warn: vi.fn(),
    }
    const ipc = {
      getProject: vi.fn((projectId) => Promise.resolve({ id: projectId })),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const selection = loadProjectSelectionData('p-wsl', ipc, {
      logger,
      projectPath: '\\\\wsl.localhost\\Ubuntu\\home\\user\\repo',
      daemonStatus: 'connected',
      batchKind: 'blocking',
    })

    await vi.advanceTimersByTimeAsync(26)
    await selection

    const sectionEvents = logger.info.mock.calls
      .map((call) => call[1])
      .filter((payload) => payload?.event === 'project.selection.section.completed')

    expect(sectionEvents).toHaveLength(6)
    const commitsEvent = sectionEvents.find((payload) => payload.section_key === 'commits')
    const detailEvent = sectionEvents.find((payload) => payload.section_key === 'detail')

    expect(commitsEvent.provider_route).toBe('daemon_provider')
    expect(commitsEvent.blocking).toBe(true)
    expect(commitsEvent.deferred).toBe(false)
    expect(detailEvent.provider_route).toBe('db')

    const batchCompleted = logger.info.mock.calls
      .map((call) => call[1])
      .find((payload) => payload?.event === 'project.selection.batch.completed')

    expect(batchCompleted.project_id).toBe('p-wsl')
    expect(batchCompleted.failed_section_count).toBe(0)
    expect(batchCompleted.blocking).toBe(true)
  })

  it('marks deferred provider sections as local fallback when daemon is unavailable', async () => {
    vi.useFakeTimers()

    const logger = {
      info: vi.fn(),
      warn: vi.fn(),
    }
    const ipc = {
      getProject: vi.fn((projectId) => Promise.resolve({ id: projectId })),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const prefetch = prefetchProjectSelectionData('p-wsl', ipc, {
      logger,
      projectPath: '\\\\wsl.localhost\\Ubuntu\\home\\user\\repo',
      daemonStatus: 'disconnected',
      batchKind: 'deferred',
    })

    await prefetch

    const readmeEvent = logger.info.mock.calls
      .map((call) => call[1])
      .find((payload) => payload?.event === 'project.selection.section.completed' && payload.section_key === 'readme')

    expect(readmeEvent.provider_route).toBe('local_provider_fallback')
    expect(readmeEvent.deferred).toBe(true)
    expect(readmeEvent.blocking).toBe(false)
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

  it('reuses a same-project in-flight batch instead of starting duplicate IPC calls', async () => {
    vi.useFakeTimers()

    function createDeferred() {
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const detail = createDeferred()
    const ipc = {
      getProject: vi.fn(() => detail.promise),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const first = loadProjectSelectionData('p1', ipc)
    await vi.advanceTimersByTimeAsync(26)

    const second = loadProjectSelectionData('p1', ipc)

    detail.resolve({ id: 'p1' })
    const [firstResult, secondResult] = await Promise.all([first, second])

    expect(firstResult).toBe(secondResult)
    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
  })

  it('prefetches a project batch and reuses it for the subsequent selection', async () => {
    function createDeferred() {
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const detail = createDeferred()
    const ipc = {
      getProject: vi.fn(() => detail.promise),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const prefetched = prefetchProjectSelectionData('p2', ipc)
    const selected = loadProjectSelectionData('p2', ipc)

    detail.resolve({ id: 'p2' })
    const [prefetchResult, selectedResult] = await Promise.all([prefetched, selected])

    expect(prefetchResult).toBe(selectedResult)
    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
  })

  it('cancels a superseded debounced batch when the next selection reuses a prefetched project', async () => {
    vi.useFakeTimers()

    function createDeferred() {
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const p2Detail = createDeferred()
    const ipc = {
      getProject: vi.fn((projectId) => (
        projectId === 'p2'
          ? p2Detail.promise
          : Promise.resolve({ id: projectId })
      )),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const superseded = loadProjectSelectionData('p1', ipc)
    const prefetched = prefetchProjectSelectionData('p2', ipc)
    const selected = loadProjectSelectionData('p2', ipc)

    p2Detail.resolve({ id: 'p2' })
    const [supersededResult, prefetchedResult, selectedResult] = await Promise.all([
      superseded,
      prefetched,
      selected,
    ])
    await vi.advanceTimersByTimeAsync(26)

    expect(supersededResult).toBe(prefetchedResult)
    expect(selectedResult).toBe(prefetchedResult)
    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getProject).toHaveBeenCalledWith('p2')
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
  })

  it('loadDeferredProjectSelectionData reuses the prefetched deferred batch', async () => {
    function createDeferred() {
      let resolve
      const promise = new Promise((res) => {
        resolve = res
      })
      return { promise, resolve }
    }

    const detail = createDeferred()
    const ipc = {
      getProject: vi.fn(() => detail.promise),
      getRecentCommits: vi.fn(() => Promise.resolve([])),
      getLatestSession: vi.fn(() => Promise.resolve(null)),
      listSessions: vi.fn(() => Promise.resolve([])),
      getReadme: vi.fn(() => Promise.resolve(null)),
      getRelationships: vi.fn(() => Promise.resolve([])),
    }

    const prefetched = prefetchProjectSelectionData('p4', ipc)
    const deferredLoaded = loadDeferredProjectSelectionData('p4', ipc)

    detail.resolve({ id: 'p4' })
    const [prefetchResult, deferredResult] = await Promise.all([prefetched, deferredLoaded])

    expect(prefetchResult).toBe(deferredResult)
    expect(ipc.getProject).toHaveBeenCalledTimes(1)
    expect(ipc.getRecentCommits).toHaveBeenCalledTimes(1)
    expect(ipc.getLatestSession).toHaveBeenCalledTimes(1)
    expect(ipc.listSessions).toHaveBeenCalledTimes(1)
    expect(ipc.getReadme).toHaveBeenCalledTimes(1)
    expect(ipc.getRelationships).toHaveBeenCalledTimes(1)
  })
})
