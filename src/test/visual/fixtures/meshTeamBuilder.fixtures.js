function createRole({
  roleId,
  name,
  kind,
  cliTool,
  model,
  behaviorSummary,
} = {}) {
  return {
    roleId,
    name,
    kind,
    cliTool,
    model,
    behaviorSummary,
  }
}

function createPreset({
  presetId,
  name,
  description,
  roleCount,
  agentCount,
  tools,
  builtIn = true,
} = {}) {
  return {
    presetId,
    name,
    description,
    roleCount,
    agentCount,
    tools,
    builtIn,
  }
}

function createScenario({
  name,
  theme,
  mode = 'setup',
  teamName = 'taurhaus-team',
  teamConfig,
  roleTemplates,
  presets,
  availableProjects,
  expected,
} = {}) {
  return {
    name,
    theme,
    mode,
    teamName,
    teamConfig,
    roleTemplates,
    presets,
    availableProjects,
    expected,
  }
}

const roleTemplates = [
  createRole({
    roleId: 'lead-claude',
    name: 'Claude Orchestrator',
    kind: 'lead',
    cliTool: 'claude',
    model: 'claude-opus-4.5',
    behaviorSummary: 'Routes planning and escalation.',
  }),
  createRole({
    roleId: 'lead-codex',
    name: 'Codex Product Lead',
    kind: 'lead',
    cliTool: 'codex',
    model: 'gpt-5.4 high',
    behaviorSummary: 'Keeps execution moving.',
  }),
  createRole({
    roleId: 'agent-codex',
    name: 'Codex Developer',
    kind: 'agent',
    cliTool: 'codex',
    model: 'gpt-5.4 high',
    behaviorSummary: 'Implements backend and tooling work.',
  }),
  createRole({
    roleId: 'agent-research',
    name: 'Claude Researcher',
    kind: 'agent',
    cliTool: 'claude',
    model: 'claude-opus-4.5',
    behaviorSummary: 'Finds sources and validates decisions.',
  }),
  createRole({
    roleId: 'agent-review',
    name: 'Claude Reviewer',
    kind: 'agent',
    cliTool: 'claude',
    model: 'claude-opus-4.5',
    behaviorSummary: 'Reviews quality and release risk.',
  }),
]

const presets = [
  createPreset({
    presetId: 'dev-team',
    name: 'Dev Team',
    description: 'Lead plus two developers.',
    roleCount: 3,
    agentCount: 2,
    tools: ['claude', 'codex'],
  }),
  createPreset({
    presetId: 'full-team',
    name: 'Full Team',
    description: 'Lead, architect, and two developers.',
    roleCount: 4,
    agentCount: 3,
    tools: ['claude', 'codex'],
  }),
  createPreset({
    presetId: 'research-team',
    name: 'Research Team',
    description: 'Lead, researcher, and developer.',
    roleCount: 3,
    agentCount: 2,
    tools: ['claude', 'codex'],
  }),
]

const availableProjects = [
  { id: '/projects/taurhaus', name: 'Taurhaus' },
  { id: '/projects/mir', name: 'MIR' },
]

export const draft_board_empty_light = createScenario({
  name: 'draft_board_empty_light',
  theme: 'light',
  mode: 'empty',
  teamName: 'taurhaus-team',
  teamConfig: {
    description: '',
    lead: null,
    agents: [],
  },
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Draft Board', 'Pick a lead role', 'Preset Chips', 'Catalog'],
    catalogCollapsed: 'false',
  },
})

export const draft_board_populated_dark = createScenario({
  name: 'draft_board_populated_dark',
  theme: 'dark',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: {
    description: 'Parallel delivery sweep for Mesh polish.',
    lead: {
      id: 'lead',
      name: 'team-lead',
      roleId: 'lead-claude',
      roleName: 'Claude Orchestrator',
      tool: 'claude',
      model: 'claude-opus-4.5',
      projectId: '/projects/taurhaus',
    },
    agents: [
      {
        id: 'agent-1',
        name: 'dev-1',
        roleId: 'agent-codex',
        roleName: 'Codex Developer',
        tool: 'codex',
        model: 'gpt-5.4 high',
        projectId: '/projects/taurhaus',
      },
      {
        id: 'agent-2',
        name: 'reviewer',
        roleId: 'agent-review',
        roleName: 'Claude Reviewer',
        tool: 'claude',
        model: 'claude-opus-4.5',
        projectId: '/projects/mir',
      },
    ],
  },
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Draft Board', 'One lead is steering the roster.', '2 assigned', 'Collapse Catalog'],
    catalogCollapsed: 'true',
  },
})

export const meshTeamBuilderScenarios = [
  draft_board_empty_light,
  draft_board_populated_dark,
]
