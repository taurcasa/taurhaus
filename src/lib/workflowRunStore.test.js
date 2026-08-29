import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ipc.js', () => ({
  listWorkflowRuns: vi.fn(),
  getWorkflowRun: vi.fn(),
}))

const { getWorkflowRun, listWorkflowRuns } = await import('./ipc.js')
const {
  isWorkflowRunCollapsed,
  resetWorkflowRunsForTest,
  toggleWorkflowRun,
  watchWorkflowSession,
  workflowSessionRuns,
} = await import('./workflowRunStore.svelte.js')

function summary(overrides = {}) {
  return {
    run_id: 'wf_live',
    name: 'feature-pr',
    phases: ['Implement'],
    status: 'live',
    started_at: 2000,
    finished_at: null,
    totals: { agents: 1, done: 0, tokens: null, tool_calls: null, duration_ms: null },
    ...overrides,
  }
}

function detail(overrides = {}) {
  return {
    ...summary(),
    agents: [
      {
        agent_id: 'agent-1',
        label: null,
        phase: null,
        model: 'claude-opus-5',
        state: 'running',
        prompt_preview: 'Implement the feature',
        last_tool: 'Bash',
        tokens: 100,
        tool_calls: 1,
        last_write_at: 5,
      },
    ],
    ...overrides,
  }
}

describe('workflowRunStore', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    resetWorkflowRunsForTest()
    listWorkflowRuns.mockReset()
    getWorkflowRun.mockReset()
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(detail())
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('lists a watched session once and orders runs newest first', async () => {
    listWorkflowRuns.mockResolvedValue([
      summary({ run_id: 'old', status: 'completed', started_at: 1000 }),
      summary({ run_id: 'new', status: 'completed', started_at: 3000 }),
    ])

    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)

    expect(listWorkflowRuns).toHaveBeenCalledExactlyOnceWith('sess-1')
    expect(workflowSessionRuns('sess-1').runs.map((run) => run.run_id)).toEqual(['new', 'old'])
    expect(workflowSessionRuns('sess-1').loaded).toBe(true)
    unwatch()
  })

  it('fetches the full run for a live run and keeps it fresh every two seconds', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])

    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)

    expect(getWorkflowRun).toHaveBeenCalledExactlyOnceWith('sess-1', 'wf_live')
    expect(workflowSessionRuns('sess-1').runs[0].agents).toHaveLength(1)

    await vi.advanceTimersByTimeAsync(2000)
    expect(listWorkflowRuns).toHaveBeenCalledTimes(2)
    expect(getWorkflowRun).toHaveBeenCalledTimes(2)

    unwatch()
  })

  it('stops polling once no run is live any more', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])
    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)

    listWorkflowRuns.mockResolvedValue([summary({ status: 'completed', finished_at: 9000 })])
    await vi.advanceTimersByTimeAsync(2000)
    const listCalls = listWorkflowRuns.mock.calls.length

    await vi.advanceTimersByTimeAsync(10_000)
    expect(listWorkflowRuns).toHaveBeenCalledTimes(listCalls)
    unwatch()
  })

  it('stops fetching a live run the viewer collapsed, and resumes on expand', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])
    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)
    expect(isWorkflowRunCollapsed('sess-1', 'wf_live')).toBe(false)

    toggleWorkflowRun('sess-1', 'wf_live')
    expect(isWorkflowRunCollapsed('sess-1', 'wf_live')).toBe(true)

    const detailCalls = getWorkflowRun.mock.calls.length
    await vi.advanceTimersByTimeAsync(10_000)
    expect(getWorkflowRun).toHaveBeenCalledTimes(detailCalls)

    toggleWorkflowRun('sess-1', 'wf_live')
    await vi.advanceTimersByTimeAsync(0)
    expect(getWorkflowRun.mock.calls.length).toBeGreaterThan(detailCalls)
    unwatch()
  })

  it('runs one poll for two watchers of the same session', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])
    const first = watchWorkflowSession('sess-1')
    const second = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)

    expect(listWorkflowRuns).toHaveBeenCalledTimes(1)

    await vi.advanceTimersByTimeAsync(2000)
    expect(listWorkflowRuns).toHaveBeenCalledTimes(2)

    first()
    second()
  })

  it('stops polling when the last watcher leaves', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])
    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)
    unwatch()

    const listCalls = listWorkflowRuns.mock.calls.length
    await vi.advanceTimersByTimeAsync(10_000)
    expect(listWorkflowRuns).toHaveBeenCalledTimes(listCalls)
  })

  it('keeps the last good runs when a poll fails and records why', async () => {
    listWorkflowRuns.mockResolvedValue([summary()])
    const unwatch = watchWorkflowSession('sess-1')
    await vi.advanceTimersByTimeAsync(0)

    listWorkflowRuns.mockRejectedValue(new Error('daemon unavailable'))
    await vi.advanceTimersByTimeAsync(2000)

    const state = workflowSessionRuns('sess-1')
    expect(state.runs).toHaveLength(1)
    expect(state.error).toBe('daemon unavailable')
    unwatch()
  })

  it('never calls the backend without a session id', async () => {
    const unwatch = watchWorkflowSession('')
    await vi.advanceTimersByTimeAsync(2000)

    expect(listWorkflowRuns).not.toHaveBeenCalled()
    expect(workflowSessionRuns('').runs).toEqual([])
    unwatch()
  })

  it('reports an unwatched session as empty and unloaded', () => {
    expect(workflowSessionRuns('nobody')).toEqual({ runs: [], loaded: false, error: null })
  })
})
