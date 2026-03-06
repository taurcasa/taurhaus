import { createAgentMembers, createConnection, createLeadMember } from './builders.js'

function createMeshCanvasScenario({
  name,
  theme,
  mode = 'runtime',
  canvasSize = { width: 900, height: 520 },
  agentCount,
  agentOverrides = [],
} = {}) {
  const lead = createLeadMember({
    projectId: 'taurhaus',
    position: {
      x: Math.round(canvasSize.width / 2),
      y: Math.round(canvasSize.height * 0.3),
    },
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

export const runtime_threeAgents_light = createMeshCanvasScenario({
  name: 'runtime_threeAgents_light',
  theme: 'light',
  agentCount: 3,
})

export const runtime_oneAgent_dark = createMeshCanvasScenario({
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
    { id: 'developer3', name: 'developer3', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    {
      id: 'mesh-expert',
      name: 'mesh-expert',
      tool: 'gemini',
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
    { id: 'developer3', name: 'developer3', tool: 'codex', model: 'gpt-5.4 high', status: 'active' },
    {
      id: 'mesh-expert',
      name: 'mesh-expert',
      tool: 'gemini',
      model: '2.5-pro',
      status: 'active',
      projectId: '/home/user/projects/mesh',
      isCrossProject: true,
      projectLabel: 'mesh',
    },
  ],
})

export const runtime_sevenAgents_dark = createMeshCanvasScenario({
  name: 'runtime_sevenAgents_dark',
  theme: 'dark',
  agentCount: 7,
})

export const runtime_eightAgents_dark = createMeshCanvasScenario({
  name: 'runtime_eightAgents_dark',
  theme: 'dark',
  agentCount: 8,
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
  empty_noAgents_light,
]
