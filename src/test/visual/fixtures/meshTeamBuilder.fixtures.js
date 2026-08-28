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
  viewport = { width: 1100, height: 1200 },
  pinnedRoleIds = [],
  expandCatalogAfterMount = false,
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
    viewport,
    pinnedRoleIds,
    expandCatalogAfterMount,
    expected,
  }
}

function createLead({
  id = 'lead',
  name = 'team-lead',
  roleId = 'lead-claude',
  roleName = 'Claude Orchestrator',
  tool = 'claude',
  model = 'claude-opus-4.5',
  projectId = '/projects/taurhaus',
} = {}) {
  return { id, name, roleId, roleName, tool, model, projectId }
}

function createAgent({
  id,
  name,
  roleId,
  roleName,
  tool,
  model,
  projectId = '/projects/taurhaus',
} = {}) {
  return { id, name, roleId, roleName, tool, model, projectId }
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
  createRole({
    roleId: 'agent-antigravity',
    name: 'Antigravity Researcher',
    kind: 'agent',
    cliTool: 'agy',
    model: 'gemini-3.7-flash-high',
    behaviorSummary: 'Explores sources and design references.',
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
  { id: '/projects/atlas', name: 'Atlas' },
]

const emptyTeam = {
  description: '',
  lead: null,
  agents: [],
}

const devTeamRoster = {
  description: 'Preset-loaded implementation team.',
  lead: createLead(),
  agents: [
    createAgent({
      id: 'agent-1',
      name: 'dev-1',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }),
    createAgent({
      id: 'agent-2',
      name: 'dev-2',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
      projectId: '/projects/mir',
    }),
  ],
}

const partialRoster = {
  description: 'Lead, implementation, and review are in place.',
  lead: createLead(),
  agents: [
    createAgent({
      id: 'agent-1',
      name: 'builder',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }),
    createAgent({
      id: 'agent-2',
      name: 'reviewer',
      roleId: 'agent-review',
      roleName: 'Claude Reviewer',
      tool: 'claude',
      model: 'claude-opus-4.5',
      projectId: '/projects/mir',
    }),
  ],
}

const fullTeamRoster = {
  description: 'Full release roster staged and ready to initialize.',
  lead: createLead(),
  agents: [
    createAgent({
      id: 'agent-1',
      name: 'architect',
      roleId: 'agent-codex',
      roleName: 'Codex Architect',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }),
    createAgent({
      id: 'agent-2',
      name: 'dev-1',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
    }),
    createAgent({
      id: 'agent-3',
      name: 'dev-2',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
      projectId: '/projects/mir',
    }),
    createAgent({
      id: 'agent-4',
      name: 'researcher',
      roleId: 'agent-research',
      roleName: 'Claude Researcher',
      tool: 'claude',
      model: 'claude-opus-4.5',
      projectId: '/projects/atlas',
    }),
  ],
}

const roster_builder_empty_state = createScenario({
  name: 'roster_builder_empty_state',
  theme: 'dark',
  mode: 'empty',
  teamName: 'taurhaus-team',
  teamConfig: emptyTeam,
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Available Roles', 'Your Team', 'Choose a lead role to anchor the team.', 'Quick start'],
    catalogCollapsed: 'false',
  },
})

const roster_builder_partial_state = createScenario({
  name: 'roster_builder_partial_state',
  theme: 'light',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: partialRoster,
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Available Roles', 'Your Team', '3 members', 'Lead, implementation, and review are in place.'],
    catalogCollapsed: 'false',
  },
})

const roster_builder_ready_state = createScenario({
  name: 'roster_builder_ready_state',
  theme: 'dark',
  mode: 'setup',
  teamName: 'release-train-team',
  teamConfig: fullTeamRoster,
  roleTemplates,
  presets,
  availableProjects,
  viewport: { width: 1280, height: 1320 },
  expected: {
    labels: ['Available Roles', 'Your Team', '5 members', 'Initialize Team'],
    catalogCollapsed: 'false',
  },
})

export const meshTeamBuilderScenarios = [
  roster_builder_empty_state,
  roster_builder_partial_state,
  roster_builder_ready_state,
]
