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
  accountStates,
  viewport = { width: 1100, height: 1200 },
  pinnedRoleIds = [],
  expandCatalogAfterMount = false,
  expandedMemberNames = [],
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
    accountStates,
    viewport,
    pinnedRoleIds,
    expandCatalogAfterMount,
    expandedMemberNames,
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
  accountId = null,
  projectId = '/projects/taurhaus',
} = {}) {
  return { id, name, roleId, roleName, tool, model, accountId, projectId }
}

function createAgent({
  id,
  name,
  roleId,
  roleName,
  tool,
  model,
  accountId = null,
  projectId = '/projects/taurhaus',
} = {}) {
  return { id, name, roleId, roleName, tool, model, accountId, projectId }
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
  createRole({
    roleId: 'agent-grok',
    name: 'Grok Developer',
    kind: 'agent',
    cliTool: 'grok',
    model: 'grok-4.6',
    behaviorSummary: 'Implements scoped changes and reports verification.',
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

const accountStates = {
  claude: {
    accounts: [{ id: 'claude-team', display_name: 'Team Pro', label: 'team@example.com', logged_in: true }],
    defaultAccountId: 'claude-team',
    degraded: false,
  },
  codex: {
    accounts: [
      { id: 'codex-work', display_name: 'Work', label: 'work@example.com', logged_in: true },
      { id: 'codex-personal', display_name: 'Personal', label: 'personal@example.com', logged_in: true },
    ],
    defaultAccountId: 'codex-work',
    degraded: false,
  },
  grok: {
    accounts: [{ id: 'grok-main', display_name: 'Main', label: 'main@example.com', logged_in: true }],
    defaultAccountId: 'grok-main',
    degraded: false,
  },
}

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
      accountId: 'codex-work',
    }),
    createAgent({
      id: 'agent-2',
      name: 'dev-2',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
      accountId: 'codex-personal',
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
      accountId: 'codex-work',
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
      accountId: 'codex-work',
    }),
    createAgent({
      id: 'agent-2',
      name: 'dev-1',
      roleId: 'agent-codex',
      roleName: 'Codex Developer',
      tool: 'codex',
      model: 'gpt-5.4 high',
      accountId: 'codex-personal',
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
  accountStates,
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
  accountStates,
  expandedMemberNames: ['lead', 'builder'],
  expected: {
    labels: ['Available Roles', 'Your Team', '3 members', 'Lead, implementation, and review are in place.', 'Team account · Team Pro', 'Work'],
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
  accountStates,
  expandedMemberNames: ['lead', 'dev-1'],
  viewport: { width: 1280, height: 1320 },
  expected: {
    labels: ['Available Roles', 'Your Team', '5 members', 'Initialize Team', 'Team account · Team Pro', 'Personal'],
    catalogCollapsed: 'false',
  },
})

export const meshTeamBuilderScenarios = [
  roster_builder_empty_state,
  roster_builder_partial_state,
  roster_builder_ready_state,
]
