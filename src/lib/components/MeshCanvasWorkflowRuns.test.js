import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  listWorkflowRuns: vi.fn(),
  getWorkflowRun: vi.fn(),
}))

const { getWorkflowRun, listWorkflowRuns } = await import('../ipc.js')
const { resetWorkflowRunsForTest } = await import('../workflowRunStore.svelte.js')
const MeshCanvas = (await import('./MeshCanvas.svelte')).default

let previousResizeObserver

beforeAll(() => {
  previousResizeObserver = globalThis.ResizeObserver
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterAll(() => {
  if (previousResizeObserver) {
    globalThis.ResizeObserver = previousResizeObserver
    return
  }
  delete globalThis.ResizeObserver
})

const LEAD = {
  id: 'lead-1',
  name: 'team-lead',
  tool: 'claude',
  model: 'opus',
  status: 'active',
}

function run(overrides = {}) {
  return {
    run_id: 'wf_live',
    name: 'feature-pr',
    phases: ['Implement'],
    status: 'live',
    started_at: 2000,
    finished_at: null,
    agents: [
      {
        agent_id: 'agent-1',
        label: 'implementer',
        phase: 'Implement',
        model: 'claude-opus-5',
        state: 'running',
        prompt_preview: 'Implement the feature',
        last_tool: 'Bash',
        tokens: 8434,
        tool_calls: 3,
        last_write_at: 5,
      },
    ],
    totals: { agents: 1, done: 0, tokens: 8434, tool_calls: 3, duration_ms: null },
    ...overrides,
  }
}

describe('MeshCanvas workflow runs', () => {
  beforeEach(() => {
    resetWorkflowRunsForTest()
    listWorkflowRuns.mockReset()
    getWorkflowRun.mockReset()
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(run())
  })

  afterEach(() => {
    resetWorkflowRunsForTest()
  })

  it('draws no tree for nodes with no runs', () => {
    render(MeshCanvas, {
      props: { lead: LEAD, agents: [{ id: 'a', name: 'a', tool: 'codex' }], mode: 'runtime' },
    })

    expect(screen.queryByTestId('workflow-run-tree')).not.toBeInTheDocument()
    expect(listWorkflowRuns).not.toHaveBeenCalled()
  })

  it('draws the runs a caller handed the node without asking the backend', () => {
    render(MeshCanvas, {
      props: {
        lead: { ...LEAD, workflowRuns: [run()] },
        agents: [],
        mode: 'runtime',
      },
    })

    expect(screen.getByTestId('workflow-run-tree')).toBeInTheDocument()
    expect(screen.getByTestId('workflow-run-agent')).toHaveTextContent('implementer')
    expect(listWorkflowRuns).not.toHaveBeenCalled()
  })

  it('watches the session of a node that carries one and draws what comes back', async () => {
    listWorkflowRuns.mockResolvedValue([run()])

    render(MeshCanvas, {
      props: {
        lead: { ...LEAD, sessionId: 'sess-1' },
        agents: [],
        mode: 'runtime',
      },
    })

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-agent')).toHaveTextContent('implementer')
    })
    expect(listWorkflowRuns).toHaveBeenCalledWith('sess-1')
  })

  it('collapses a live run when its header is clicked', async () => {
    listWorkflowRuns.mockResolvedValue([run()])

    render(MeshCanvas, {
      props: { lead: { ...LEAD, session_id: 'sess-1' }, agents: [], mode: 'runtime' },
    })

    await waitFor(() => expect(screen.getByTestId('workflow-run-agent')).toBeInTheDocument())
    await fireEvent.click(screen.getByTestId('workflow-run-header'))

    await waitFor(() => {
      expect(screen.queryByTestId('workflow-run-agent')).not.toBeInTheDocument()
    })
  })

  // The runtime canvas re-renders its members on every team-status refresh. A
  // fresh array carrying the same sessions is not a reason to re-subscribe, and
  // every re-subscribe costs a `list_workflow_runs`.
  it('does not re-subscribe when the member array changes identity', async () => {
    listWorkflowRuns.mockResolvedValue([run()])

    const { rerender } = render(MeshCanvas, {
      props: { lead: { ...LEAD, sessionId: 'sess-1' }, agents: [], mode: 'runtime' },
    })
    await waitFor(() => expect(screen.getByTestId('workflow-run-agent')).toBeInTheDocument())
    const calls = listWorkflowRuns.mock.calls.length

    await rerender({
      lead: { ...LEAD, sessionId: 'sess-1' },
      agents: [{ id: 'a', name: 'a', tool: 'codex' }],
      mode: 'runtime',
    })
    await waitFor(() => expect(screen.getByTestId('mesh-node-agent')).toBeInTheDocument())

    expect(listWorkflowRuns).toHaveBeenCalledTimes(calls)
  })

  it('grows the canvas so a tree at the bottom stays inside it', () => {
    const { container } = render(MeshCanvas, {
      props: {
        lead: LEAD,
        agents: [{ id: 'a', name: 'a', tool: 'codex', workflowRuns: [run()] }],
        mode: 'runtime',
      },
    })

    const canvas = screen.getByTestId('mesh-canvas')
    const minHeight = Number.parseFloat(canvas.style.minHeight)
    const tree = container.querySelector('[data-testid="workflow-run-tree"]')
    const treeBottom = Number.parseFloat(tree.style.top) + Number.parseFloat(tree.style.height)

    expect(minHeight).toBeGreaterThanOrEqual(treeBottom)
  })
})
