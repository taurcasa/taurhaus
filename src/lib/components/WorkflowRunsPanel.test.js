import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  getProjectTasks: vi.fn(),
  listWorkflowRuns: vi.fn(),
  getWorkflowRun: vi.fn(),
  workflowLedgerRow: vi.fn(),
}))

const { getProjectTasks, getWorkflowRun, listWorkflowRuns, workflowLedgerRow } =
  await import('../ipc.js')
const WorkflowRunsPanel = (await import('./WorkflowRunsPanel.svelte')).default

const COMPLETED = {
  run_id: 'wf_done',
  name: 'feature-pr',
  description: 'Implement, review, and gate a feature',
  phases: ['Implement', 'Review', 'Gate'],
  status: 'completed',
  started_at: 3000,
  finished_at: 141_000,
  totals: { agents: 3, done: 3, tokens: 12_400, tool_calls: 9, duration_ms: 138_000 },
}

const OLDER = {
  run_id: 'wf_old',
  name: 'docs-sweep',
  description: 'Sweep the docs',
  phases: ['Sweep'],
  status: 'failed',
  started_at: 1000,
  finished_at: 2000,
  totals: { agents: 1, done: 0, tokens: null, tool_calls: null, duration_ms: 1000 },
}

const DETAIL = {
  ...COMPLETED,
  agents: [
    {
      agent_id: 'agent-1',
      label: 'implementer',
      phase: 'Implement',
      model: 'claude-opus-5',
      state: 'done',
      prompt_preview: 'Implement the feature',
      last_tool: 'Bash',
      tokens: 8434,
      tool_calls: 3,
      last_write_at: 5,
      result_preview: 'done',
    },
  ],
  result: {},
}

function renderPanel(props = {}) {
  return render(WorkflowRunsPanel, {
    props: { projectId: 'proj-1', sessions: [], ...props },
  })
}

describe('WorkflowRunsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getProjectTasks.mockResolvedValue({ tasks: [] })
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(DETAIL)
    workflowLedgerRow.mockResolvedValue('| Mock | Codex | Opus | 1 | 0 | tbd |')
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
  })

  it('stays out of the way when the project has no runs', async () => {
    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-1'))
    expect(screen.queryByTestId('overview-workflow-runs')).not.toBeInTheDocument()
  })

  it('asks nothing when the project has no known sessions', async () => {
    renderPanel()

    await waitFor(() => expect(getProjectTasks).toHaveBeenCalledWith('proj-1'))
    expect(listWorkflowRuns).not.toHaveBeenCalled()
  })

  it('lists runs from every known session, newest first', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [{ id: '1', session_id: 'sess-2' }] })
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-1' ? [OLDER] : [COMPLETED])
    )

    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => {
      expect(screen.getAllByTestId('workflow-run-row')).toHaveLength(2)
    })

    const rows = screen.getAllByTestId('workflow-run-row')
    expect(rows[0]).toHaveTextContent('feature-pr')
    expect(rows[0]).toHaveTextContent('Completed')
    expect(rows[0]).toHaveTextContent('3/3')
    expect(rows[0]).toHaveTextContent('12.4k')
    expect(rows[0]).toHaveTextContent('2m 18s')
    expect(rows[1]).toHaveTextContent('docs-sweep')
    expect(rows[1]).toHaveTextContent('Failed')
  })

  it('shows the agent table of the run you pick', async () => {
    listWorkflowRuns.mockResolvedValue([COMPLETED])
    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    await fireEvent.click(screen.getByTestId('workflow-run-row'))

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-detail')).toBeInTheDocument()
    })
    expect(getWorkflowRun).toHaveBeenCalledWith('sess-1', 'wf_done')

    const agentRow = screen.getByTestId('workflow-detail-agent')
    expect(agentRow).toHaveTextContent('implementer')
    expect(agentRow).toHaveTextContent('Implement')
    expect(agentRow).toHaveTextContent('claude-opus-5')
    expect(agentRow).toHaveTextContent('Bash')
    expect(agentRow).toHaveTextContent('8.4k')
  })

  it('copies the ledger row of the selected run', async () => {
    listWorkflowRuns.mockResolvedValue([COMPLETED])
    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    await fireEvent.click(screen.getByTestId('workflow-run-row'))

    const copyButton = await screen.findByTestId('workflow-copy-ledger')
    await waitFor(() => expect(copyButton).toBeEnabled())
    await fireEvent.click(copyButton)

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        '| Mock | Codex | Opus | 1 | 0 | tbd |'
      )
    })
    expect(copyButton).toHaveTextContent('Copied')
  })

  it('disables the copy action for a run with no ledger row', async () => {
    listWorkflowRuns.mockResolvedValue([COMPLETED])
    workflowLedgerRow.mockResolvedValue(null)
    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    await fireEvent.click(screen.getByTestId('workflow-run-row'))

    const copyButton = await screen.findByTestId('workflow-copy-ledger')
    await waitFor(() => expect(copyButton).toBeDisabled())
    expect(copyButton).toHaveAttribute('title', expect.stringContaining('no ledger row'))
  })

  // A session snapshot arrives on every daemon update and hands this panel a
  // fresh array each time. Only a change in which sessions those are is a
  // reason to ask the backend again.
  it('does not re-query when a new session array carries the same sessions', async () => {
    listWorkflowRuns.mockResolvedValue([COMPLETED])
    const { rerender } = renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    const calls = listWorkflowRuns.mock.calls.length

    await rerender({ projectId: 'proj-1', sessions: [{ session_id: 'sess-1' }] })
    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())

    expect(listWorkflowRuns).toHaveBeenCalledTimes(calls)

    await rerender({ projectId: 'proj-1', sessions: [{ session_id: 'sess-9' }] })
    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-9'))
  })

  it('keeps the list when one session cannot be read', async () => {
    getProjectTasks.mockResolvedValue({ tasks: [{ id: '1', session_id: 'sess-2' }] })
    listWorkflowRuns.mockImplementation((sessionId) =>
      sessionId === 'sess-1' ? Promise.reject(new Error('gone')) : Promise.resolve([COMPLETED])
    )

    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => {
      expect(screen.getAllByTestId('workflow-run-row')).toHaveLength(1)
    })
  })
})
