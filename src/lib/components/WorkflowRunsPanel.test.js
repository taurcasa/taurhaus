import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../ipc.js', () => ({
  getArchivedSessions: vi.fn(),
  getProjectTasks: vi.fn(),
  listWorkflowRuns: vi.fn(),
  getWorkflowRun: vi.fn(),
  workflowLedgerRow: vi.fn(),
}))

const { getArchivedSessions, getProjectTasks, getWorkflowRun, listWorkflowRuns, workflowLedgerRow } =
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
    getArchivedSessions.mockResolvedValue({ sessions: [], errors: [] })
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

describe('WorkflowRunsPanel theming', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getArchivedSessions.mockResolvedValue({ sessions: [], errors: [] })
    getProjectTasks.mockResolvedValue({ tasks: [] })
    listWorkflowRuns.mockResolvedValue([COMPLETED])
    getWorkflowRun.mockResolvedValue(DETAIL)
    workflowLedgerRow.mockResolvedValue(null)
  })

  // The status pills switch colour with the panel's own `dark` prop, not with a
  // global `html.dark` class: every other component in this codebase themes
  // itself from the prop, and a surface rendered dark inside a light document
  // (or the reverse) has to come out right.
  it('carries its own dark marker rather than depending on a global class', async () => {
    const { rerender } = render(WorkflowRunsPanel, {
      props: { projectId: 'proj-1', sessions: [{ session_id: 'sess-1' }], dark: true },
    })

    await waitFor(() => {
      expect(screen.getByTestId('overview-workflow-runs')).toHaveClass('is-dark')
    })

    await rerender({ projectId: 'proj-1', sessions: [{ session_id: 'sess-1' }], dark: false })
    expect(screen.getByTestId('overview-workflow-runs')).not.toHaveClass('is-dark')
  })
})

const LIVE = {
  run_id: 'wf_live',
  name: 'feature-pr',
  description: 'Implement, review, and gate a feature',
  phases: ['Implement'],
  status: 'live',
  started_at: 9000,
  finished_at: null,
  totals: { agents: 3, done: 1, tokens: 2100, tool_calls: 4, duration_ms: null },
}

function liveSession(sessionId, liveRuns = 1) {
  return {
    session_id: sessionId,
    workflow_activity: { live_runs: liveRuns, last_write_at: 1_787_949_436_814 },
  }
}

describe('WorkflowRunsPanel run lifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getArchivedSessions.mockResolvedValue({ sessions: [], errors: [] })
    getProjectTasks.mockResolvedValue({ tasks: [] })
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(DETAIL)
    workflowLedgerRow.mockResolvedValue(null)
  })

  // Regression: d010cee keyed the reload on the set of session ids alone. A run
  // starts and ends inside a session that is already listed, so the panel
  // returned early and a run that began after the tab was open never appeared.
  it('picks up a run that starts in a session it already knows', async () => {
    const { rerender } = renderPanel({ sessions: [{ session_id: 'sess-1' }] })
    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-1'))
    expect(screen.queryByTestId('workflow-run-row')).not.toBeInTheDocument()

    listWorkflowRuns.mockResolvedValue([LIVE])
    await rerender({ projectId: 'proj-1', sessions: [liveSession('sess-1')] })

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('Live')
    })
  })

  // Regression: the same key left a finished run showing as live forever
  // (d010cee).
  it('notices a live run finishing in a session it already knows', async () => {
    listWorkflowRuns.mockResolvedValue([LIVE])
    const { rerender } = renderPanel({ sessions: [liveSession('sess-1')] })
    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('Live')
    })

    listWorkflowRuns.mockResolvedValue([{ ...LIVE, status: 'completed', finished_at: 20_000 }])
    await rerender({ projectId: 'proj-1', sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('Completed')
    })
  })

  it('keeps the open run selected while a live run refreshes', async () => {
    listWorkflowRuns.mockResolvedValue([LIVE])
    const { rerender } = renderPanel({ sessions: [liveSession('sess-1')] })
    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    await fireEvent.click(screen.getByTestId('workflow-run-row'))
    await waitFor(() => expect(screen.getByTestId('workflow-run-detail')).toBeInTheDocument())

    await rerender({ projectId: 'proj-1', sessions: [liveSession('sess-1', 2)] })
    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledTimes(2))

    expect(screen.getByTestId('workflow-run-detail')).toBeInTheDocument()
  })
})

describe('WorkflowRunsPanel session coverage', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    getArchivedSessions.mockResolvedValue({ sessions: [], errors: [] })
    getProjectTasks.mockResolvedValue({ tasks: [] })
    listWorkflowRuns.mockResolvedValue([])
    getWorkflowRun.mockResolvedValue(DETAIL)
    workflowLedgerRow.mockResolvedValue(null)
  })

  // Regression: d010cee named a project's sessions from the live snapshot and
  // `get_project_tasks`, and the latter returns only unarchived tasks. Once a
  // session ended and its tasks archived, its runs left the history — which is
  // exactly the history this panel exists to show.
  it('asks the archived sessions of the project too', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [{ session_id: 'sess-archived', started_at: '2026-08-01T10:00:00Z' }],
      errors: [],
    })
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-archived' ? [COMPLETED] : [])
    )

    renderPanel({ sessions: [] })

    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-archived'))
    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('feature-pr')
    })
  })

  // Regression: 1663e40 collected the live sessions by `session_id`, a field
  // `DisplaySession` strips, so a workflow running right now in a session with
  // no task and no archive record never reached the history at all.
  it('asks the session the frontend snapshot names', async () => {
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-display' ? [COMPLETED] : [])
    )

    renderPanel({ sessions: [{ workflow_session_id: 'sess-display' }] })

    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-display'))
    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('feature-pr')
    })
  })

  it('keeps the list when the archived sessions cannot be read', async () => {
    getArchivedSessions.mockRejectedValue(new Error('no cache'))
    listWorkflowRuns.mockResolvedValue([COMPLETED])

    renderPanel({ sessions: [{ session_id: 'sess-1' }] })

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('feature-pr')
    })
  })

  // Regression: 2772530 concatenated live, open-task and archived sessions and
  // cut the first 24 — and open tasks arrive ordered by source/source_key/task
  // id, not by recency. Two dozen of them pushed a newer archived session past
  // the cut, so its runs were never asked for.
  it('orders every candidate by its own timestamp before it cuts the list', async () => {
    getProjectTasks.mockResolvedValue({
      tasks: Array.from({ length: 30 }, (_unused, index) => ({
        session_id: `sess-task-${index}`,
        updated_at: '2026-02-01T09:00:00Z',
      })),
    })
    getArchivedSessions.mockResolvedValue({
      sessions: [{ session_id: 'sess-recent', ended_at: '2026-03-01T09:00:00Z' }],
      errors: [],
    })
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-recent' ? [COMPLETED] : [])
    )

    renderPanel({ sessions: [] })

    await waitFor(() => expect(listWorkflowRuns).toHaveBeenCalledWith('sess-recent'))
    expect(listWorkflowRuns.mock.calls[0][0]).toBe('sess-recent')
    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('feature-pr')
    })
  })

  // Regression: 2772530 stopped at the first 24 candidates whatever came back,
  // so a project whose only workflow ran in an older session had no history at
  // all and no way to reach it.
  it('keeps looking back when the newest sessions have no runs', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: [
        ...Array.from({ length: 30 }, (_unused, index) => ({
          session_id: `sess-quiet-${index}`,
          ended_at: '2026-03-02T10:00:00Z',
        })),
        { session_id: 'sess-ancient', ended_at: '2026-01-01T10:00:00Z' },
      ],
      errors: [],
    })
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-ancient' ? [COMPLETED] : [])
    )

    renderPanel({ sessions: [] })

    await waitFor(() => {
      expect(screen.getByTestId('workflow-run-row')).toHaveTextContent('feature-pr')
    })
    expect(listWorkflowRuns).toHaveBeenCalledWith('sess-ancient')
  })

  // Regression: the cap was applied to a set that had no order, so a project
  // with more archived sessions than the cap could lose its live one (d010cee).
  it('asks the newest sessions first and says so when it stopped short', async () => {
    getArchivedSessions.mockResolvedValue({
      sessions: Array.from({ length: 40 }, (_unused, index) => ({
        session_id: `sess-old-${index}`,
      })),
      errors: [],
    })
    listWorkflowRuns.mockImplementation((sessionId) =>
      Promise.resolve(sessionId === 'sess-now' ? [COMPLETED] : [])
    )

    renderPanel({ sessions: [{ session_id: 'sess-now' }] })

    await waitFor(() => expect(screen.getByTestId('workflow-run-row')).toBeInTheDocument())
    const asked = listWorkflowRuns.mock.calls.map(([sessionId]) => sessionId)
    expect(asked[0]).toBe('sess-now')
    expect(asked).toContain('sess-old-0')
    expect(asked.length).toBeLessThan(41)
    expect(screen.getByTestId('overview-workflow-runs')).toHaveTextContent(
      `newest ${asked.length} sessions`
    )
  })
})
