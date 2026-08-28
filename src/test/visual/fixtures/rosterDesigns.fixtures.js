/**
 * Roster Design mockup scenarios.
 * Each design has 3 states: empty, partial (lead + 1-2 agents), ready (full team).
 */

export const rosterDesignScenarios = [
  { name: 'Empty — no members', state: 'empty', theme: 'dark' },
  { name: 'Partial — lead + 2 agents', state: 'partial', theme: 'dark' },
  { name: 'Ready — full team', state: 'ready', theme: 'dark' },
  { name: 'Empty (light)', state: 'empty', theme: 'light' },
  { name: 'Partial (light)', state: 'partial', theme: 'light' },
  { name: 'Ready (light)', state: 'ready', theme: 'light' },
]

export const MOCK_ROLES = [
  { id: 'orchestrator', name: 'Orchestrator', kind: 'lead', tool: 'claude', model: 'opus', summary: 'Coordinates team delivery and sequences work across specialists.' },
  { id: 'codex-lead', name: 'Codex Lead', kind: 'lead', tool: 'codex', model: 'gpt-5.4', summary: 'Drives implementation with broad codebase context.' },
  { id: 'developer', name: 'Developer', kind: 'agent', tool: 'codex', model: 'gpt-5.4', summary: 'Implements features and fixes with focused execution.' },
  { id: 'architect', name: 'Architect', kind: 'agent', tool: 'claude', model: 'opus', summary: 'Reviews architecture decisions and guards module boundaries.' },
  { id: 'designer', name: 'UI Specialist', kind: 'agent', tool: 'agy', model: '3.1-pro', summary: 'Designs interfaces with creative ownership and visual polish.' },
  { id: 'qa', name: 'QA Engineer', kind: 'agent', tool: 'codex', model: 'gpt-5.4', summary: 'Writes tests, finds regressions, and validates acceptance criteria.' },
  { id: 'researcher', name: 'Researcher', kind: 'agent', tool: 'claude', model: 'sonnet', summary: 'Explores options, reads docs, and produces technical summaries.' },
  { id: 'reviewer', name: 'Code Reviewer', kind: 'agent', tool: 'claude', model: 'opus', summary: 'Reviews PRs for correctness, style, and security.' },
  { id: 'grok-developer', name: 'Grok Developer', kind: 'agent', tool: 'grok', model: 'grok-4.6', summary: 'Implements scoped changes on the xAI harness with TDD.' },
]

export const MOCK_PRESETS = [
  { name: 'Pair', agents: 1, leads: 1, tools: ['claude', 'codex'] },
  { name: 'Dev Team', agents: 2, leads: 1, tools: ['claude', 'codex'] },
  { name: 'Full Team', agents: 3, leads: 1, tools: ['claude', 'codex', 'agy'] },
  { name: 'Research', agents: 2, leads: 1, tools: ['claude'] },
  { name: 'Grok Pair', agents: 1, leads: 1, tools: ['claude', 'grok'] },
]

export const MOCK_TEAM_PARTIAL = {
  name: 'taurhaus-team',
  description: 'Multi-tool dev team for parallel implementation.',
  lead: MOCK_ROLES[0],
  agents: [MOCK_ROLES[2], MOCK_ROLES[3]],
}

export const MOCK_TEAM_READY = {
  name: 'taurhaus-team',
  description: 'Multi-tool dev team for parallel implementation.',
  lead: MOCK_ROLES[0],
  agents: [MOCK_ROLES[2], MOCK_ROLES[3], MOCK_ROLES[4], MOCK_ROLES[5], MOCK_ROLES[8]],
}
