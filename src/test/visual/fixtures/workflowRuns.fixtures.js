/**
 * Workflow run fixtures shared by every surface that draws one: the canvas run
 * tree, the sidebar badge and the Overview run-history panel. Shapes follow
 * `docs/architecture/workflow-runs.md`, including the `null`s the scanner
 * genuinely returns for a live agent it could not place in a phase.
 */

function agent(overrides = {}) {
  return {
    agent_id: 'agent-1',
    label: null,
    phase: null,
    model: 'claude-opus-5',
    state: 'running',
    prompt_preview: 'Implement the run tree',
    last_tool: 'Edit',
    tokens: 8434,
    tool_calls: 3,
    last_write_at: 1787949436814,
    result_preview: null,
    ...overrides,
  }
}

export const LIVE_RUN = {
  run_id: 'wf_live_01',
  name: 'feature-pr',
  description: 'Implement, review, and gate a feature',
  phases: ['Implement', 'Review', 'Gate'],
  status: 'live',
  started_at: 1787949435335,
  finished_at: null,
  agents: [
    agent({ agent_id: 'implementer', label: 'implementer', phase: 'Implement', state: 'done', tokens: 18_400 }),
    agent({ agent_id: 'lens-opus', label: 'lens-opus', phase: 'Review', state: 'running', model: 'claude-opus-5', last_tool: 'Read', tokens: 6200 }),
    agent({ agent_id: 'lens-codex', label: 'lens-codex', phase: 'Review', state: 'queued', model: 'gpt-5.6', last_tool: null, tokens: null }),
  ],
  totals: { agents: 3, done: 1, tokens: 24_600, tool_calls: 21, duration_ms: null },
  result: null,
}

export const UNPHASED_LIVE_RUN = {
  run_id: 'wf_live_02',
  name: 'research-sweep',
  description: 'Three researchers answer one question',
  phases: ['Research', 'Synthesis'],
  status: 'live',
  started_at: 1787949431000,
  finished_at: null,
  agents: [
    agent({ agent_id: 'researcher-1', prompt_preview: 'Answer the question read-only', tokens: 3400 }),
    agent({ agent_id: 'researcher-2', prompt_preview: 'Answer the question read-only', tokens: null, last_tool: 'Grep' }),
  ],
  totals: { agents: 3, done: 0, tokens: null, tool_calls: null, duration_ms: null },
  result: null,
}

export const FINISHED_RUN = {
  run_id: 'wf_done_01',
  name: 'docs-sweep',
  description: 'Sweep the docs against the code',
  phases: ['Sweep', 'Verify', 'Gate'],
  status: 'completed',
  started_at: 1787948435335,
  finished_at: 1787948573335,
  agents: [
    agent({ agent_id: 'sweeper', label: 'sweeper', phase: 'Sweep', state: 'done', tokens: 12_400 }),
    agent({ agent_id: 'verifier', label: 'verifier', phase: 'Verify', state: 'done', model: 'gpt-5.6', tokens: 7100 }),
  ],
  totals: { agents: 2, done: 2, tokens: 19_500, tool_calls: 34, duration_ms: 138_000 },
  result: {},
}

const FAILED_RUN = {
  run_id: 'wf_failed_01',
  name: 'small-change',
  description: 'One implementer, one lens, one fix round',
  phases: ['Implement', 'Review'],
  status: 'failed',
  started_at: 1787947435335,
  finished_at: 1787947495335,
  agents: [
    agent({ agent_id: 'implementer', label: 'implementer', phase: 'Implement', state: 'failed', tokens: 2100 }),
  ],
  totals: { agents: 2, done: 0, tokens: 2100, tool_calls: 4, duration_ms: 60_000 },
  result: {},
}

function summaryOf(run) {
  const summary = { ...run }
  delete summary.agents
  delete summary.result
  return summary
}

const HISTORY = [LIVE_RUN, FINISHED_RUN, FAILED_RUN].map(summaryOf)

const DETAIL_BY_RUN_ID = {
  [LIVE_RUN.run_id]: LIVE_RUN,
  [FINISHED_RUN.run_id]: FINISHED_RUN,
  [FAILED_RUN.run_id]: FAILED_RUN,
}

function createPanelScenario({
  name,
  theme,
  runs = HISTORY,
  selectRunName = null,
  ledgerRow = null,
  emptyNote = null,
}) {
  return {
    name,
    theme,
    selectRunName,
    emptyNote,
    projectId: 'project-workflow',
    sessions: [{ session_id: 'sess-workflow-1' }],
    ipc: {
      getProjectTasks: { tasks: [], errors: [] },
      listWorkflowRuns: runs,
      getWorkflowRun: (_sessionId, runId) =>
        Promise.resolve(DETAIL_BY_RUN_ID[runId] ?? FINISHED_RUN),
      workflowLedgerRow: ledgerRow,
    },
  }
}

const history_list_dark = createPanelScenario({
  name: 'history_list_dark',
  theme: 'dark',
})

const history_list_light = createPanelScenario({
  name: 'history_list_light',
  theme: 'light',
})

const history_selectedRun_light = createPanelScenario({
  name: 'history_selectedRun_light',
  theme: 'light',
  selectRunName: 'docs-sweep',
  ledgerRow: '| Docs sweep | Opus | Codex gpt-5.6 | 1 | 0 | tbd |',
})

const history_selectedRun_noLedger_dark = createPanelScenario({
  name: 'history_selectedRun_noLedger_dark',
  theme: 'dark',
  selectRunName: 'small-change',
})

// The section hides itself when a project's sessions ran no workflows; the note
// is host chrome standing in for the rest of the Overview tab around it.
const history_empty_light = createPanelScenario({
  name: 'history_empty_light',
  theme: 'light',
  runs: [],
  emptyNote: 'Overview continues here — the Workflow runs section renders nothing.',
})

export const workflowRunsScenarios = [
  history_list_dark,
  history_list_light,
  history_selectedRun_light,
  history_selectedRun_noLedger_dark,
  history_empty_light,
]
