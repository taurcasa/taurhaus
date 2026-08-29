import { describe, expect, it } from 'vitest'

import {
  collectWorkflowSessionIds,
  formatRunDuration,
  formatTokens,
  runListRow,
  runTreeDescriptor,
  runTreeModel,
  workflowSessionId,
} from './workflowRuns.js'

function agent(overrides = {}) {
  return {
    agent_id: 'agent-1',
    label: null,
    phase: null,
    model: 'claude-opus-5',
    state: 'running',
    prompt_preview: 'Implement the feature',
    last_tool: 'Bash',
    tokens: 8434,
    tool_calls: 3,
    last_write_at: 1787949436814,
    result_preview: null,
    ...overrides,
  }
}

function run(overrides = {}) {
  return {
    run_id: 'wf_1',
    name: 'feature-pr',
    description: 'Implement, review, and gate a feature',
    phases: ['Implement', 'Review', 'Gate'],
    status: 'live',
    started_at: 1787949435335,
    finished_at: null,
    agents: [agent()],
    totals: { agents: 1, done: 0, tokens: 8434, tool_calls: 3, duration_ms: null },
    result: null,
    ...overrides,
  }
}

describe('workflowSessionId', () => {
  it('reads either spelling and trims', () => {
    expect(workflowSessionId({ session_id: ' abc ' })).toBe('abc')
    expect(workflowSessionId({ sessionId: 'def' })).toBe('def')
  })

  it('is empty for a record without one', () => {
    expect(workflowSessionId({})).toBe('')
    expect(workflowSessionId(null)).toBe('')
    expect(workflowSessionId({ session_id: 42 })).toBe('')
  })
})

describe('collectWorkflowSessionIds', () => {
  it('dedupes ids across sources and keeps first-seen order', () => {
    expect(
      collectWorkflowSessionIds([{ session_id: 'a' }, { sessionId: 'b' }], [{ session_id: 'a' }])
    ).toEqual(['a', 'b'])
  })

  it('skips records without an id', () => {
    expect(collectWorkflowSessionIds([{}, null, { session_id: '' }])).toEqual([])
  })

  it('tolerates a non-array source', () => {
    expect(collectWorkflowSessionIds(null, undefined, [{ session_id: 'a' }])).toEqual(['a'])
  })
})

describe('formatTokens', () => {
  it('formats known counts compactly', () => {
    expect(formatTokens(834)).toBe('834')
    expect(formatTokens(8434)).toBe('8.4k')
    expect(formatTokens(12_000)).toBe('12k')
    expect(formatTokens(1_240_000)).toBe('1.2M')
  })

  it('returns null when the scanner could not count exactly', () => {
    expect(formatTokens(null)).toBeNull()
    expect(formatTokens(undefined)).toBeNull()
    expect(formatTokens('lots')).toBeNull()
  })
})

describe('formatRunDuration', () => {
  it('prefers the summary total', () => {
    expect(formatRunDuration(run({ totals: { agents: 1, done: 1, duration_ms: 2337 } }))).toBe('2s')
  })

  it('falls back to the finished window', () => {
    expect(
      formatRunDuration(run({
        started_at: 1000,
        finished_at: 139_000,
        totals: { agents: 1, done: 1, duration_ms: null },
      }))
    ).toBe('2m 18s')
  })

  it('formats hours', () => {
    expect(formatRunDuration(run({ totals: { duration_ms: 3_720_000 } }))).toBe('1h 2m')
  })

  it('is null for a live run with no total', () => {
    expect(formatRunDuration(run())).toBeNull()
  })
})

describe('runTreeModel', () => {
  it('renders one implicit group when no agent carries a phase', () => {
    const model = runTreeModel(run())

    expect(model.name).toBe('feature-pr')
    expect(model.isLive).toBe(true)
    expect(model.groups).toHaveLength(1)
    expect(model.groups[0].title).toBeNull()
    expect(model.groups[0].agents[0].label).toBe('Implement the feature')
    expect(model.groups[0].agents[0].model).toBe('claude-opus-5')
    expect(model.groups[0].agents[0].lastTool).toBe('Bash')
    expect(model.groups[0].agents[0].tokensLabel).toBe('8.4k')
  })

  it('counts one row per rendered phase title plus one per agent', () => {
    const model = runTreeModel(run({
      agents: [
        agent({ agent_id: 'a', phase: 'Implement' }),
        agent({ agent_id: 'b', phase: 'Review' }),
        agent({ agent_id: 'c', phase: 'Review' }),
      ],
    }))

    expect(model.groups.map((group) => group.title)).toEqual(['Implement', 'Review'])
    expect(model.rowCount).toBe(5)
  })

  it('keeps the declared phase order and trails phase-less agents', () => {
    const model = runTreeModel(run({
      agents: [
        agent({ agent_id: 'a', phase: 'Gate' }),
        agent({ agent_id: 'b' }),
        agent({ agent_id: 'c', phase: 'Implement' }),
      ],
    }))

    expect(model.groups.map((group) => group.title)).toEqual(['Implement', 'Gate', null])
  })

  it('groups a phase the script never declared after the declared ones', () => {
    const model = runTreeModel(run({
      agents: [agent({ agent_id: 'a', phase: 'Cleanup' }), agent({ agent_id: 'b', phase: 'Gate' })],
    }))

    expect(model.groups.map((group) => group.title)).toEqual(['Gate', 'Cleanup'])
  })

  it('prefers the label over the prompt preview and truncates a long preview', () => {
    const model = runTreeModel(run({
      agents: [
        agent({ agent_id: 'a', label: 'implementer' }),
        agent({ agent_id: 'b', prompt_preview: 'x'.repeat(80) }),
      ],
    }))

    expect(model.groups[0].agents[0].label).toBe('implementer')
    expect(model.groups[0].agents[1].label).toHaveLength(48)
    expect(model.groups[0].agents[1].label.endsWith('…')).toBe(true)
  })

  it('falls back to the agent id when there is neither label nor prompt', () => {
    const model = runTreeModel(run({ agents: [agent({ prompt_preview: '   ' })] }))

    expect(model.groups[0].agents[0].label).toBe('agent-1')
  })

  it('collapses a finished run to one summary line', () => {
    const model = runTreeModel(run({
      status: 'completed',
      finished_at: 1787949437672,
      agents: [agent({ state: 'done' })],
      totals: { agents: 3, done: 3, tokens: 12_400, tool_calls: 9, duration_ms: 138_000 },
    }))

    expect(model.isLive).toBe(false)
    expect(model.summary).toBe('feature-pr · 3/3 done · 12.4k tokens · 2m 18s')
    expect(model.rowCount).toBe(0)
  })

  it('omits totals the scanner could not count', () => {
    const model = runTreeModel(run({
      status: 'failed',
      totals: { agents: 2, done: 1, tokens: null, tool_calls: null, duration_ms: null },
    }))

    expect(model.summary).toBe('feature-pr · 1/2 done')
    expect(model.status).toBe('failed')
  })

  it('returns null for a missing run', () => {
    expect(runTreeModel(null)).toBeNull()
    expect(runTreeModel({})).toBeNull()
  })
})

describe('runListRow', () => {
  it('describes a completed summary row', () => {
    const row = runListRow({
      run_id: 'wf_1',
      name: 'feature-pr',
      description: 'Implement, review, and gate a feature',
      phases: ['Implement', 'Review', 'Gate'],
      status: 'completed',
      started_at: 1787949435335,
      finished_at: 1787949437672,
      totals: { agents: 3, done: 3, tokens: 12_400, tool_calls: 9, duration_ms: 138_000 },
    })

    expect(row).toMatchObject({
      runId: 'wf_1',
      name: 'feature-pr',
      status: 'completed',
      statusLabel: 'Completed',
      isLive: false,
      phasesLabel: 'Implement · Review · Gate',
      doneLabel: '3/3',
      tokensLabel: '12.4k',
      durationLabel: '2m 18s',
    })
  })

  it('marks a live row and leaves an uncounted duration blank', () => {
    const row = runListRow({
      run_id: 'wf_2',
      name: 'docs-sweep',
      phases: [],
      status: 'live',
      started_at: 1787949435335,
      totals: { agents: 2, done: 0, tokens: null, tool_calls: null, duration_ms: null },
    })

    expect(row.isLive).toBe(true)
    expect(row.statusLabel).toBe('Live')
    expect(row.phasesLabel).toBe('')
    expect(row.tokensLabel).toBeNull()
    expect(row.durationLabel).toBeNull()
  })

  it('labels an unrecognised status without guessing', () => {
    const row = runListRow({ run_id: 'wf_3', name: 'x', status: 'unknown', started_at: 1 })
    expect(row.statusLabel).toBe('Unknown')
  })
})

describe('runTreeDescriptor', () => {
  it('is null for a node with no runs', () => {
    expect(runTreeDescriptor([])).toBeNull()
    expect(runTreeDescriptor(null)).toBeNull()
    expect(runTreeDescriptor([{}])).toBeNull()
  })

  it('counts the rows of every expanded live run and one header per run', () => {
    const descriptor = runTreeDescriptor([
      run({ agents: [agent({ agent_id: 'a' }), agent({ agent_id: 'b' })] }),
      run({ run_id: 'wf_2', status: 'completed', agents: [] }),
    ])

    expect(descriptor).toEqual({ rowCount: 2, runCount: 2, collapsed: false })
  })

  it('drops the rows of a run the viewer collapsed', () => {
    const descriptor = runTreeDescriptor([run()], ['wf_1'])

    expect(descriptor).toEqual({ rowCount: 0, runCount: 1, collapsed: true })
  })
})
