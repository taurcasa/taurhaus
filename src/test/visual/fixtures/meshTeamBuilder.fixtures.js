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
    roleId: 'agent-gemini',
    name: 'Gemini Researcher',
    kind: 'agent',
    cliTool: 'gemini',
    model: 'gemini-2.5-pro',
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

export const draft_board_empty_state = createScenario({
  name: 'draft_board_empty_state',
  theme: 'light',
  mode: 'empty',
  teamName: 'taurhaus-team',
  teamConfig: emptyTeam,
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Draft Board', 'Pick a lead role', 'Presets', 'Catalog'],
    catalogCollapsed: 'false',
  },
})

export const draft_board_preset_applied = createScenario({
  name: 'draft_board_preset_applied',
  theme: 'dark',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: devTeamRoster,
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Preset-loaded implementation team.', '2 assigned', 'Expand Catalog'],
    catalogCollapsed: 'true',
  },
})

export const draft_board_partially_built = createScenario({
  name: 'draft_board_partially_built',
  theme: 'light',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: partialRoster,
  roleTemplates,
  presets,
  availableProjects,
  expandCatalogAfterMount: true,
  expected: {
    labels: ['Lead, implementation, and review are in place.', 'Collapse Catalog', 'Catalog'],
    catalogCollapsed: 'false',
  },
})

export const draft_board_catalog_collapsed = createScenario({
  name: 'draft_board_catalog_collapsed',
  theme: 'dark',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: partialRoster,
  roleTemplates,
  presets,
  availableProjects,
  expected: {
    labels: ['Lead, implementation, and review are in place.', 'Expand Catalog'],
    catalogCollapsed: 'true',
  },
})

export const draft_board_catalog_with_favorites = createScenario({
  name: 'draft_board_catalog_with_favorites',
  theme: 'light',
  mode: 'setup',
  teamName: 'taurhaus-team',
  teamConfig: emptyTeam,
  roleTemplates,
  presets,
  availableProjects,
  pinnedRoleIds: ['lead-codex', 'agent-research', 'agent-review'],
  expected: {
    labels: ['Favorites', 'Codex Product Lead', 'Claude Researcher', 'Claude Reviewer'],
    catalogCollapsed: 'false',
  },
})

export const draft_board_full_team_ready = createScenario({
  name: 'draft_board_full_team_ready',
  theme: 'dark',
  mode: 'setup',
  teamName: 'release-train-team',
  teamConfig: fullTeamRoster,
  roleTemplates,
  presets,
  availableProjects,
  viewport: { width: 1100, height: 1480 },
  expected: {
    labels: ['Full release roster staged and ready to initialize.', '4 assigned', 'Initialize Team'],
    catalogCollapsed: 'true',
  },
})

export const meshTeamBuilderScenarios = [
  draft_board_empty_state,
  draft_board_preset_applied,
  draft_board_partially_built,
  draft_board_catalog_collapsed,
  draft_board_catalog_with_favorites,
  draft_board_full_team_ready,
]
