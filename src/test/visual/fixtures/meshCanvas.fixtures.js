import { createAgentMembers, createConnection, createLeadMember } from './builders.js'
import { FINISHED_RUN, LIVE_RUN, UNPHASED_LIVE_RUN } from './workflowRuns.fixtures.js'

function createMeshCanvasScenario({
  name,
  theme,
  mode = 'runtime',
  canvasSize = { width: 900, height: 520 },
  agentCount,
  agentOverrides = [],
  leadOverrides = {},
} = {}) {
  const lead = createLeadMember({
    projectId: 'taurhaus',
    position: {
      x: Math.round(canvasSize.width / 2),
      y: Math.round(canvasSize.height * 0.3),
    },
    ...leadOverrides,
  })
  const agents = createAgentMembers(agentCount, { canvasSize }).map((agent, index) => ({
    ...agent,
    ...(agentOverrides[index] ?? {}),
  }))

  return {
    name,
    theme,
    mode,
    canvasSize,
    members: [lead, ...agents],
    connections: agents.map((agent) => createConnection(lead.id, agent.id)),
  }
}

const runtime_threeAgents_light = createMeshCanvasScenario({
  name: 'runtime_threeAgents_light',
  theme: 'light',
  agentCount: 3,
})

const runtime_oneAgent_dark = createMeshCanvasScenario({
  name: 'runtime_oneAgent_dark',
  theme: 'dark',
  agentCount: 1,
})

export const runtime_threeAgents_dark = createMeshCanvasScenario({
  name: 'runtime_threeAgents_dark',
  theme: 'dark',
  agentCount: 3,
})

export const runtime_fiveAgents_dark = createMeshCanvasScenario({
  name: 'runtime_fiveAgents_dark',
  theme: 'dark',
  agentCount: 5,
  agentOverrides: [
    { id: 'architect', name: 'architect', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'developer1', name: 'developer1', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'developer2', name: 'developer2', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'grok-dev', name: 'grok-dev', tool: 'grok', model: 'grok-4.6', status: 'idle' },
    {
      id: 'mesh-expert',
      name: 'mesh-expert',
      tool: 'agy',
      model: '2.5-pro',
      status: 'active',
      projectId: '/home/user/projects/mesh',
      isCrossProject: true,
      projectLabel: 'mesh',
    },
  ],
})

export const runtime_fiveAgents_light = createMeshCanvasScenario({
  name: 'runtime_fiveAgents_light',
  theme: 'light',
  agentCount: 5,
  agentOverrides: [
    { id: 'architect', name: 'architect', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'developer1', name: 'developer1', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'developer2', name: 'developer2', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    { id: 'grok-dev', name: 'grok-dev', tool: 'grok', model: 'grok-4.6', status: 'idle' },
    {
      id: 'mesh-expert',
      name: 'mesh-expert',
      tool: 'agy',
      model: '2.5-pro',
      status: 'active',
      projectId: '/home/user/projects/mesh',
      isCrossProject: true,
      projectLabel: 'mesh',
    },
  ],
})

const runtime_sevenAgents_dark = createMeshCanvasScenario({
  name: 'runtime_sevenAgents_dark',
  theme: 'dark',
  agentCount: 7,
})

const runtime_eightAgents_dark = createMeshCanvasScenario({
  name: 'runtime_eightAgents_dark',
  theme: 'dark',
  agentCount: 8,
})

// The lead is the node that invokes a workflow, so its tree is the one the
// canvas has to place without drawing over the agents beneath it.
export const runtime_workflowLiveTree_dark = createMeshCanvasScenario({
  name: 'runtime_workflowLiveTree_dark',
  theme: 'dark',
  agentCount: 3,
  leadOverrides: { workflowRuns: [LIVE_RUN] },
})

export const runtime_workflowLiveTree_light = createMeshCanvasScenario({
  name: 'runtime_workflowLiveTree_light',
  theme: 'light',
  agentCount: 3,
  leadOverrides: { workflowRuns: [LIVE_RUN] },
})

// One agent whose live run has no phases the scanner could place, next to one
// whose runs have finished and collapsed to a line each.
export const runtime_workflowMixedRuns_dark = createMeshCanvasScenario({
  name: 'runtime_workflowMixedRuns_dark',
  theme: 'dark',
  agentCount: 3,
  agentOverrides: [
    { workflowRuns: [UNPHASED_LIVE_RUN] },
    {},
    { workflowRuns: [FINISHED_RUN] },
  ],
})

export const runtime_accountStates_dark = createMeshCanvasScenario({
  name: 'runtime_accountStates_dark',
  theme: 'dark',
  agentCount: 3,
  agentOverrides: [
    { accountId: 'codex-work', accountLabel: 'Work', accountApplied: true },
    { accountId: 'agy-main', accountLabel: 'Main', accountApplied: false },
    { accountId: 'claude-team', accountLabel: 'Team Pro', accountFallbackFrom: 'Legacy' },
  ],
})

export const runtime_accountStates_light = createMeshCanvasScenario({
  name: 'runtime_accountStates_light',
  theme: 'light',
  agentCount: 3,
  agentOverrides: [
    { accountId: 'codex-work', accountLabel: 'Work', accountApplied: true },
    { accountId: 'agy-main', accountLabel: 'Main', accountApplied: false },
    { accountId: 'claude-team', accountLabel: 'Team Pro', accountFallbackFrom: 'Legacy' },
  ],
})

export const empty_noAgents_light = createMeshCanvasScenario({
  name: 'empty_noAgents_light',
  theme: 'light',
  mode: 'setup',
  agentCount: 0,
})

export const meshCanvasScenarios = [
  runtime_threeAgents_light,
  runtime_oneAgent_dark,
  runtime_threeAgents_dark,
  runtime_fiveAgents_dark,
  runtime_fiveAgents_light,
  runtime_sevenAgents_dark,
  runtime_eightAgents_dark,
  runtime_workflowLiveTree_dark,
  runtime_workflowLiveTree_light,
  runtime_workflowMixedRuns_dark,
  runtime_accountStates_dark,
  runtime_accountStates_light,
  empty_noAgents_light,
]
