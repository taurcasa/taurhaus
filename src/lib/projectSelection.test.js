import { afterEach, describe, expect, it, vi } from 'vitest'

import { createProjectSelectionRequests, withFallback } from './projectSelection.js'

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
})
