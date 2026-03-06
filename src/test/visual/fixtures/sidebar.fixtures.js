function createProject({
  id,
  name,
  path,
  activityState = 'active',
  branch = 'main',
  isDirty = false,
} = {}) {
  return {
    id,
    name,
    path,
    activityState,
    branch,
    isDirty,
  }
}

function createSession({
  cli_tool = 'claude',
  state = 'active',
  tmux_session = 'team',
  tmux_window = '1',
  tmux_pane = '%3',
  _duration = 8 * 60_000,
  _lastTransition = Date.now() - 90_000,
  project_unattributed_active = false,
} = {}) {
  return {
    cli_tool,
    state,
    tmux_session,
    tmux_window,
    tmux_pane,
    _duration,
    _lastTransition,
    project_unattributed_active,
  }
}

function createScenario({
  name,
  theme,
  projects,
  selectedProject = null,
  daemonStatus = 'connected',
  sessionStore = { sessionsByProject: {}, sessionByProject: {} },
  expected,
  compareAgainst = null,
}) {
  return {
    name,
    theme,
    projects,
    selectedProject,
    daemonStatus,
    sessionStore,
    expected,
    compareAgainst,
  }
}

const activeClaudeProject = createProject({
  id: 'project-active-claude',
  name: 'Active Claude Project',
  path: '/projects/active-claude',
  activityState: 'active',
  branch: 'main',
})

const multiToolProject = createProject({
  id: 'project-multi-tool',
  name: 'Multi Tool Runtime',
  path: '/projects/multi-tool-runtime',
  activityState: 'active',
  branch: 'mesh/runtime',
})

const idleCodexProject = createProject({
  id: 'project-idle-codex',
  name: 'Idle Codex Queue',
  path: '/projects/idle-codex-queue',
  activityState: 'active',
  branch: 'feature/review',
})

const dirtyNoSessionProject = createProject({
  id: 'project-dirty',
  name: 'Dirty Without Session',
  path: '/projects/dirty-without-session',
  activityState: 'stale',
  branch: 'fix/sidebar',
  isDirty: true,
})

const dormantCleanProject = createProject({
  id: 'project-dormant',
  name: 'Dormant Clean Project',
  path: '/projects/dormant-clean',
  activityState: 'dormant',
  branch: 'main',
  isDirty: false,
})

const groupedProjects = [
  createProject({
    id: 'project-group-active',
    name: 'Active Workstream',
    path: '/projects/group-active',
    activityState: 'active',
    branch: 'main',
  }),
  createProject({
    id: 'project-group-recent',
    name: 'Recent Handoff',
    path: '/projects/group-recent',
    activityState: 'recent',
    branch: 'review/handoff',
  }),
  createProject({
    id: 'project-group-stale',
    name: 'Stale Cleanup',
    path: '/projects/group-stale',
    activityState: 'stale',
    branch: 'cleanup',
    isDirty: true,
  }),
  createProject({
    id: 'project-group-dormant',
    name: 'Dormant Archive',
    path: '/projects/group-dormant',
    activityState: 'dormant',
    branch: 'main',
  }),
]

export const active_claude_selected_dark = createScenario({
  name: 'active_claude_selected_dark',
  theme: 'dark',
  projects: [activeClaudeProject],
  selectedProject: activeClaudeProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/active-claude': [
        createSession({ cli_tool: 'claude', state: 'active', tmux_window: '3', tmux_pane: '%31' }),
      ],
    },
    sessionByProject: {
      '/projects/active-claude': createSession({ cli_tool: 'claude', state: 'active', tmux_window: '3', tmux_pane: '%31' }),
    },
  },
  expected: {
    labels: ['Active Claude Project', 'Claude: running', 'Connected'],
    selectedProjectName: 'Active Claude Project',
  },
})

export const active_multiTool_dark = createScenario({
  name: 'active_multiTool_dark',
  theme: 'dark',
  projects: [multiToolProject],
  selectedProject: multiToolProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/multi-tool-runtime': [
        createSession({ cli_tool: 'claude', state: 'active', tmux_window: '1', tmux_pane: '%11' }),
        createSession({ cli_tool: 'codex', state: 'active', tmux_window: '2', tmux_pane: '%12' }),
        createSession({ cli_tool: 'gemini', state: 'active', tmux_window: '4', tmux_pane: '%14' }),
      ],
    },
    sessionByProject: {
      '/projects/multi-tool-runtime': createSession({ cli_tool: 'claude', state: 'active', tmux_window: '1', tmux_pane: '%11' }),
    },
  },
  expected: {
    labels: ['Multi Tool Runtime', 'Claude: running', 'Codex: running', 'Gemini: running'],
    selectedProjectName: 'Multi Tool Runtime',
  },
})

export const idle_codex_dark = createScenario({
  name: 'idle_codex_dark',
  theme: 'dark',
  projects: [idleCodexProject],
  selectedProject: idleCodexProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/idle-codex-queue': [
        createSession({ cli_tool: 'codex', state: 'idle', tmux_window: '6', tmux_pane: '%61' }),
      ],
    },
    sessionByProject: {
      '/projects/idle-codex-queue': createSession({ cli_tool: 'codex', state: 'idle', tmux_window: '6', tmux_pane: '%61' }),
    },
  },
  expected: {
    labels: ['Idle Codex Queue', 'Codex: idle'],
    selectedProjectName: 'Idle Codex Queue',
  },
})

export const dirty_noSession_dark = createScenario({
  name: 'dirty_noSession_dark',
  theme: 'dark',
  projects: [dirtyNoSessionProject],
  selectedProject: dirtyNoSessionProject,
  daemonStatus: 'disconnected',
  sessionStore: {
    sessionsByProject: {
      '/projects/dirty-without-session': [],
    },
    sessionByProject: {},
  },
  expected: {
    labels: ['Dirty Without Session', 'Daemon offline'],
    selectedProjectName: 'Dirty Without Session',
  },
})

export const dormant_clean_dark = createScenario({
  name: 'dormant_clean_dark',
  theme: 'dark',
  projects: [dormantCleanProject],
  selectedProject: dormantCleanProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/dormant-clean': [],
    },
    sessionByProject: {},
  },
  expected: {
    labels: ['Dormant Clean Project', 'Connected'],
    selectedProjectName: 'Dormant Clean Project',
  },
})

export const active_claude_selected_light = createScenario({
  name: 'active_claude_selected_light',
  theme: 'light',
  projects: [activeClaudeProject],
  selectedProject: activeClaudeProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/active-claude': [
        createSession({ cli_tool: 'claude', state: 'active', tmux_window: '3', tmux_pane: '%31' }),
      ],
    },
    sessionByProject: {
      '/projects/active-claude': createSession({ cli_tool: 'claude', state: 'active', tmux_window: '3', tmux_pane: '%31' }),
    },
  },
  expected: {
    labels: ['Active Claude Project', 'Claude: running', 'Connected'],
    selectedProjectName: 'Active Claude Project',
  },
  compareAgainst: 'active_claude_selected_dark',
})

export const groupHeaders_dark = createScenario({
  name: 'groupHeaders_dark',
  theme: 'dark',
  projects: groupedProjects,
  selectedProject: groupedProjects[0],
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/group-active': [
        createSession({ cli_tool: 'claude', state: 'active', tmux_window: '9', tmux_pane: '%91' }),
      ],
      '/projects/group-recent': [],
      '/projects/group-stale': [],
      '/projects/group-dormant': [],
    },
    sessionByProject: {
      '/projects/group-active': createSession({ cli_tool: 'claude', state: 'active', tmux_window: '9', tmux_pane: '%91' }),
    },
  },
  expected: {
    labels: ['ACTIVE', 'RECENT', 'STALE', 'DORMANT', 'Active Workstream', 'Recent Handoff', 'Stale Cleanup', 'Dormant Archive'],
    selectedProjectName: 'Active Workstream',
  },
})

export const sidebarScenarios = [
  active_claude_selected_dark,
  active_multiTool_dark,
  idle_codex_dark,
  dirty_noSession_dark,
  dormant_clean_dark,
  active_claude_selected_light,
  groupHeaders_dark,
]
