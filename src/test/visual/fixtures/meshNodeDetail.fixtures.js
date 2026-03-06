function createMember({
  name,
  role = 'agent',
  tool = 'claude',
  toolLabel = 'Claude',
  status = 'active',
  model = '',
  projectId = 'taurhaus',
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

export const active_claude_light = createDetailScenario({
  name: 'active_claude_light',
  theme: 'light',
  mode: 'runtime',
  focusEnabled: true,
  member: {
    ...activeClaudeMember,
    sessionId: 'sess-claude-alpha-light',
  },
})

export const idle_codex_dark = createDetailScenario({
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

export const idle_gemini_dark = createDetailScenario({
  name: 'idle_gemini_dark',
  theme: 'dark',
  mode: 'runtime',
  focusEnabled: true,
  member: createMember({
    name: 'research-gemini',
    role: 'agent',
    tool: 'gemini',
    toolLabel: 'Gemini',
    status: 'idle',
    model: '2.5-pro',
    projectId: 'taurhaus-docs',
    description: 'Tracks architecture drift and prepares review notes for handoff.',
    paneId: '%21',
    sessionId: 'sess-gemini-idle',
    sessionState: 'Waiting for follow-up',
    sessionTiming: {
      startedLabel: 'Started 32m ago',
      activeLabel: 'Idle for 6m',
    },
  }),
})

export const disconnected_dark = createDetailScenario({
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
    name: 'new-gemini-agent',
    role: 'agent',
    tool: 'gemini',
    toolLabel: 'Gemini',
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
  idle_gemini_dark,
  disconnected_dark,
  noSession_dark,
]
