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
  pid = 1,
  group_kind = 'standalone',
  group_id = null,
  group_label = null,
  member_name = null,
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
    pid,
    group_kind,
    group_id,
    group_label,
    member_name,
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

const teamRailTwoProject = createProject({
  id: 'project-team-rail-two',
  name: 'Team Rail Pair',
  path: '/projects/team-rail-pair',
  activityState: 'active',
  branch: 'mesh/pair',
})

const teamRailThreeProject = createProject({
  id: 'project-team-rail-three',
  name: 'Team Rail Trio',
  path: '/projects/team-rail-trio',
  activityState: 'active',
  branch: 'mesh/trio',
})

const teamStackMixedProject = createProject({
  id: 'project-team-stack-mixed',
  name: 'Team Stack Mixed',
  path: '/projects/team-stack-mixed',
  activityState: 'active',
  branch: 'mesh/mixed',
})

const teamPlusStandaloneProject = createProject({
  id: 'project-team-plus-standalone',
  name: 'Team Plus Standalone',
  path: '/projects/team-plus-standalone',
  activityState: 'active',
  branch: 'mesh/handoff',
})

const teamRailThresholdProject = createProject({
  id: 'project-team-rail-threshold',
  name: 'Team Rail Threshold',
  path: '/projects/team-rail-threshold',
  activityState: 'active',
  branch: 'mesh/threshold',
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

export const team_rail_two_dark = createScenario({
  name: 'team_rail_two_dark',
  theme: 'dark',
  projects: [teamRailTwoProject],
  selectedProject: teamRailTwoProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/team-rail-pair': [
        createSession({
          pid: 11,
          cli_tool: 'claude',
          state: 'active',
          group_kind: 'mesh_team',
          group_id: 'team-rail-pair',
          group_label: 'team-rail-pair',
          member_name: 'team-lead',
        }),
        createSession({
          pid: 12,
          cli_tool: 'codex',
          state: 'idle',
          tmux_pane: '%4',
          group_kind: 'mesh_team',
          group_id: 'team-rail-pair',
          group_label: 'team-rail-pair',
          member_name: 'developer2',
        }),
      ],
    },
    sessionByProject: {
      '/projects/team-rail-pair': createSession({
        pid: 11,
        cli_tool: 'claude',
        state: 'active',
        group_kind: 'mesh_team',
        group_id: 'team-rail-pair',
        group_label: 'team-rail-pair',
        member_name: 'team-lead',
      }),
    },
  },
  expected: {
    labels: ['Team Rail Pair', 'Claude: running', 'Codex: idle', 'Connected'],
    selectedProjectName: 'Team Rail Pair',
  },
})

export const team_rail_three_light = createScenario({
  name: 'team_rail_three_light',
  theme: 'light',
  projects: [teamRailThreeProject],
  selectedProject: teamRailThreeProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/team-rail-trio': [
        createSession({
          pid: 21,
          cli_tool: 'claude',
          state: 'idle',
          tmux_pane: '%21',
          group_kind: 'mesh_team',
          group_id: 'team-rail-trio',
          group_label: 'team-rail-trio',
          member_name: 'developer1',
        }),
        createSession({
          pid: 22,
          cli_tool: 'codex',
          state: 'idle',
          tmux_pane: '%22',
          group_kind: 'mesh_team',
          group_id: 'team-rail-trio',
          group_label: 'team-rail-trio',
          member_name: 'developer2',
        }),
        createSession({
          pid: 23,
          cli_tool: 'gemini',
          state: 'idle',
          tmux_pane: '%23',
          group_kind: 'mesh_team',
          group_id: 'team-rail-trio',
          group_label: 'team-rail-trio',
          member_name: 'developer3',
        }),
      ],
    },
    sessionByProject: {
      '/projects/team-rail-trio': createSession({
        pid: 21,
        cli_tool: 'claude',
        state: 'idle',
        tmux_pane: '%21',
        group_kind: 'mesh_team',
        group_id: 'team-rail-trio',
        group_label: 'team-rail-trio',
        member_name: 'developer1',
      }),
    },
  },
  expected: {
    labels: ['Team Rail Trio', 'Claude: idle', 'Codex: idle', 'Gemini: idle', 'Connected'],
    selectedProjectName: 'Team Rail Trio',
  },
})

export const team_stack_mixed_dark = createScenario({
  name: 'team_stack_mixed_dark',
  theme: 'dark',
  projects: [teamStackMixedProject],
  selectedProject: teamStackMixedProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/team-stack-mixed': [
        createSession({
          pid: 31,
          cli_tool: 'claude',
          state: 'active',
          tmux_pane: '%31',
          group_kind: 'mesh_team',
          group_id: 'team-stack-mixed',
          group_label: 'team-stack-mixed',
          member_name: 'team-lead',
        }),
        createSession({
          pid: 32,
          cli_tool: 'codex',
          state: 'idle',
          tmux_pane: '%32',
          group_kind: 'mesh_team',
          group_id: 'team-stack-mixed',
          group_label: 'team-stack-mixed',
          member_name: 'developer1',
        }),
        createSession({
          pid: 33,
          cli_tool: 'gemini',
          state: 'idle',
          tmux_pane: '%33',
          group_kind: 'mesh_team',
          group_id: 'team-stack-mixed',
          group_label: 'team-stack-mixed',
          member_name: 'developer2',
        }),
        createSession({
          pid: 34,
          cli_tool: 'codex',
          state: 'active',
          tmux_pane: '%34',
          group_kind: 'mesh_team',
          group_id: 'team-stack-mixed',
          group_label: 'team-stack-mixed',
          member_name: 'developer3',
        }),
        createSession({
          pid: 35,
          cli_tool: 'claude',
          state: 'idle',
          tmux_pane: '%35',
          group_kind: 'mesh_team',
          group_id: 'team-stack-mixed',
          group_label: 'team-stack-mixed',
          member_name: 'developer4',
        }),
      ],
    },
    sessionByProject: {
      '/projects/team-stack-mixed': createSession({
        pid: 31,
        cli_tool: 'claude',
        state: 'active',
        tmux_pane: '%31',
        group_kind: 'mesh_team',
        group_id: 'team-stack-mixed',
        group_label: 'team-stack-mixed',
        member_name: 'team-lead',
      }),
    },
  },
  expected: {
    labels: ['Team Stack Mixed', 'team-stack-mixed: 5 team sessions active', 'Connected'],
    selectedProjectName: 'Team Stack Mixed',
  },
})

export const team_plus_standalone_dark = createScenario({
  name: 'team_plus_standalone_dark',
  theme: 'dark',
  projects: [teamPlusStandaloneProject],
  selectedProject: teamPlusStandaloneProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/team-plus-standalone': [
        createSession({
          pid: 41,
          cli_tool: 'claude',
          state: 'active',
          tmux_pane: '%41',
          group_kind: 'mesh_team',
          group_id: 'team-plus-standalone',
          group_label: 'team-plus-standalone',
          member_name: 'team-lead',
        }),
        createSession({
          pid: 42,
          cli_tool: 'codex',
          state: 'idle',
          tmux_pane: '%42',
          group_kind: 'mesh_team',
          group_id: 'team-plus-standalone',
          group_label: 'team-plus-standalone',
          member_name: 'developer2',
        }),
        createSession({
          pid: 43,
          cli_tool: 'gemini',
          state: 'active',
          tmux_pane: '%43',
          group_kind: 'standalone',
        }),
      ],
    },
    sessionByProject: {
      '/projects/team-plus-standalone': createSession({
        pid: 41,
        cli_tool: 'claude',
        state: 'active',
        tmux_pane: '%41',
        group_kind: 'mesh_team',
        group_id: 'team-plus-standalone',
        group_label: 'team-plus-standalone',
        member_name: 'team-lead',
      }),
    },
  },
  expected: {
    labels: ['Team Plus Standalone', 'Claude: running', 'Codex: idle', 'Gemini: running', 'Connected'],
    selectedProjectName: 'Team Plus Standalone',
  },
})

export const team_rail_threshold_dark = createScenario({
  name: 'team_rail_threshold_dark',
  theme: 'dark',
  projects: [teamRailThresholdProject],
  selectedProject: teamRailThresholdProject,
  daemonStatus: 'connected',
  sessionStore: {
    sessionsByProject: {
      '/projects/team-rail-threshold': [
        createSession({
          pid: 51,
          cli_tool: 'claude',
          state: 'active',
          tmux_pane: '%51',
          group_kind: 'mesh_team',
          group_id: 'team-rail-threshold',
          group_label: 'team-rail-threshold',
          member_name: 'team-lead',
        }),
        createSession({
          pid: 52,
          cli_tool: 'codex',
          state: 'idle',
          tmux_pane: '%52',
          group_kind: 'mesh_team',
          group_id: 'team-rail-threshold',
          group_label: 'team-rail-threshold',
          member_name: 'developer2',
        }),
        createSession({
          pid: 53,
          cli_tool: 'gemini',
          state: 'idle',
          tmux_pane: '%53',
          group_kind: 'standalone',
        }),
        createSession({
          pid: 54,
          cli_tool: 'claude',
          state: 'active',
          tmux_pane: '%54',
          group_kind: 'standalone',
        }),
      ],
    },
    sessionByProject: {
      '/projects/team-rail-threshold': createSession({
        pid: 51,
        cli_tool: 'claude',
        state: 'active',
        tmux_pane: '%51',
        group_kind: 'mesh_team',
        group_id: 'team-rail-threshold',
        group_label: 'team-rail-threshold',
        member_name: 'team-lead',
      }),
    },
  },
  expected: {
    labels: ['Team Rail Threshold', 'team-rail-threshold: 2 team sessions active', 'Gemini: idle', 'Claude: running', 'Connected'],
    selectedProjectName: 'Team Rail Threshold',
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
  team_rail_two_dark,
  team_rail_three_light,
  team_stack_mixed_dark,
  team_plus_standalone_dark,
  team_rail_threshold_dark,
]
