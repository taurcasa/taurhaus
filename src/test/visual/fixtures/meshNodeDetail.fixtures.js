function createMember({
  name,
  role = 'agent',
  tool = 'claude',
  toolLabel = 'Claude',
  status = 'active',
  model = '',
  projectId = 'taurhaus',
  isCrossProject = false,
  projectLabel = '',
  description = '',
  paneId = '',
  sessionId = '',
  sessionState = '',
  sessionTiming = null,
} = {}) {
  return {
    name,
    role,
    tool,
    toolLabel,
    status,
    model,
    projectId,
    isCrossProject,
    projectLabel,
    description,
    paneId,
    sessionId,
    sessionState,
    sessionTiming,
  }
}

function createDetailScenario({
  name,
  theme,
  mode = 'runtime',
  focusEnabled = false,
  member,
} = {}) {
  return {
    name,
    theme,
    mode,
    focusEnabled,
    member,
  }
}

const activeClaudeMember = createMember({
  name: 'architect-alpha',
  role: 'agent',
  tool: 'claude',
  toolLabel: 'Claude',
  status: 'active',
  model: 'opus-4.1',
  projectId: 'taurhaus-ui',
  description: 'Owns runtime architecture review and incident triage for the mesh shell.',
  paneId: '%12',
  sessionId: 'sess-claude-alpha',
  sessionState: 'Responding now',
  sessionTiming: {
    startedLabel: 'Started 14m ago',
    activeLabel: 'Active for 13m 42s',
  },
})

export const active_claude_dark = createDetailScenario({
  name: 'active_claude_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: true,
  member: activeClaudeMember,
})

const active_claude_light = createDetailScenario({
  name: 'active_claude_light',
  theme: 'light',
  mode: 'runtime',
  focusEnabled: true,
  member: {
    ...activeClaudeMember,
    sessionId: 'sess-claude-alpha-light',
  },
})

const idle_codex_dark = createDetailScenario({
  name: 'idle_codex_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: true,
  member: createMember({
    name: 'frontend-codex',
    role: 'agent',
    tool: 'codex',
    toolLabel: 'Codex',
    status: 'idle',
    model: 'gpt-5.4 high',
    projectId: 'taurhaus-web',
    description: 'Maintains the shell surface and visual regression harness.',
    paneId: '%18',
    sessionId: 'sess-codex-idle',
    sessionState: 'Idle after review',
    sessionTiming: {
      startedLabel: 'Started 1h ago',
      activeLabel: 'Idle for 9m',
    },
  }),
})

const idle_agy_dark = createDetailScenario({
  name: 'idle_agy_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: true,
  member: createMember({
    name: 'research-antigravity',
    role: 'agent',
    tool: 'agy',
    toolLabel: 'Antigravity',
    status: 'idle',
    model: '2.5-pro',
    projectId: 'taurhaus-docs',
    description: 'Tracks architecture drift and prepares review notes for handoff.',
    paneId: '%21',
    sessionId: 'sess-agy-idle',
    sessionState: 'Waiting for follow-up',
    sessionTiming: {
      startedLabel: 'Started 32m ago',
      activeLabel: 'Idle for 6m',
    },
  }),
})

export const cross_project_agy_dark = createDetailScenario({
  name: 'cross_project_agy_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: true,
  member: createMember({
    name: 'mesh-expert',
    role: 'agent',
    tool: 'agy',
    toolLabel: 'Antigravity',
    status: 'active',
    model: '2.5-pro',
    projectId: '/home/user/projects/mesh',
    isCrossProject: true,
    projectLabel: 'mesh',
    description: 'Works in the mesh codebase and advises on protocol changes.',
    paneId: '%33',
    sessionId: 'sess-mesh-expert',
    sessionState: 'Reviewing remote repo flow',
    sessionTiming: {
      startedLabel: 'Started 22m ago',
      activeLabel: 'Active for 21m',
    },
  }),
})

export const cross_project_agy_light = createDetailScenario({
  name: 'cross_project_agy_light',
  theme: 'light',
  mode: 'runtime',
  focusEnabled: true,
  member: createMember({
    name: 'mesh-expert',
    role: 'agent',
    tool: 'agy',
    toolLabel: 'Antigravity',
    status: 'active',
    model: '2.5-pro',
    projectId: '/home/user/projects/mesh',
    isCrossProject: true,
    projectLabel: 'mesh',
    description: 'Works in the mesh codebase and advises on protocol changes.',
    paneId: '%33',
    sessionId: 'sess-mesh-expert',
    sessionState: 'Reviewing remote repo flow',
    sessionTiming: {
      startedLabel: 'Started 22m ago',
      activeLabel: 'Active for 21m',
    },
  }),
})

const disconnected_dark = createDetailScenario({
  name: 'disconnected_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: false,
  member: createMember({
    name: 'ops-codex',
    role: 'agent',
    tool: 'codex',
    toolLabel: 'Codex',
    status: 'offline',
    model: 'gpt-5.4 high',
    projectId: 'taurhaus-daemon',
    description: 'Session disconnected after a daemon restart; awaiting resume.',
    sessionId: 'sess-codex-disconnected',
    sessionState: 'Disconnected 3m ago',
    sessionTiming: {
      startedLabel: 'Last seen 3m ago',
      activeLabel: 'No live pane',
    },
  }),
})

export const noSession_dark = createDetailScenario({
  name: 'noSession_dark',
  theme: 'dark',
  mode: 'setup',
  focusEnabled: false,
  member: createMember({
    name: 'new-antigravity-agent',
    role: 'agent',
    tool: 'agy',
    toolLabel: 'Antigravity',
    status: 'offline',
    model: '2.5-pro',
    projectId: 'taurhaus-onboarding',
    description: 'Prepared during setup but not started yet.',
    sessionTiming: {
      startedLabel: 'No live session yet',
      activeLabel: 'Setup phase',
    },
  }),
})

export const meshNodeDetailScenarios = [
  active_claude_dark,
  active_claude_light,
  idle_codex_dark,
  idle_agy_dark,
  cross_project_agy_dark,
  cross_project_agy_light,
  disconnected_dark,
  noSession_dark,
]
