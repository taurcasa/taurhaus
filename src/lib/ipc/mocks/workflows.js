export const MOCK_WORKFLOW_RUN = {
  run_id: 'wf_mock-123',
  name: 'feature-pr',
  description: 'Implement, review, and gate a feature',
  phases: ['Implement', 'Review', 'Gate'],
  status: 'completed',
  started_at: 1787949435335,
  finished_at: 1787949437672,
  agents: [
    {
      agent_id: 'agent-implementer',
      label: 'implementer',
      phase: 'Implement',
      model: 'claude-opus-5',
      state: 'done',
      prompt_preview: 'Implement the feature',
      last_tool: 'Bash',
      tokens: 8434,
      tool_calls: 3,
      last_write_at: 1787949436814,
      result_preview: 'done',
    },
  ],
  totals: { agents: 1, done: 1, tokens: 8434, tool_calls: 3, duration_ms: 2337 },
  result: {
    ledger: {
      title: 'Mock workflow',
      size: 'feature',
      implementer: 'Codex',
      reviewers: ['Opus'],
      rounds: 1,
      majors: 0,
      findings: [],
      remaining: [],
    },
    commits: ['abc123'],
    gate: { status: 'pass' },
  },
  script_path: '/mock/session/workflows/scripts/feature-pr-wf_mock-123.js',
}

export const MOCK_WORKFLOW_RUNS = [
  Object.fromEntries(
    Object.entries(MOCK_WORKFLOW_RUN).filter(([key]) => !['agents', 'result'].includes(key))
  ),
]

export const MOCK_WORKFLOW_LEDGER_ROW = '| Mock workflow | Codex | Opus | 1 | 0 | tbd |'
