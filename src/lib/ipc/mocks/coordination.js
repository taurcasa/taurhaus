export function buildMockInitializeReport(teamName) {
  return {
    teamName,
    succeededSteps: [
      'validate_configuration',
      'create_team',
      'create_panes',
      'launch_sessions',
      'join_mesh',
      'start_daemons',
      'send_onboarding',
    ],
    failedStep: null,
    retryable: false,
    message: 'team initialized',
    steps: [
      { step: 'validate_configuration', status: 'succeeded', message: 'request validated' },
      { step: 'create_team', status: 'succeeded', message: 'team created' },
      { step: 'send_onboarding', status: 'succeeded', message: 'onboarding messages sent' },
    ],
  }
}

export function buildMockAddAgentReport(request) {
  return {
    teamName: request?.teamName ?? '',
    memberName: request?.agent?.name ?? '',
    succeededSteps: ['validate', 'create_pane', 'launch_session', 'join_mesh', 'start_daemon', 'send_onboarding', 'update_roster'],
    failedStep: null,
    retryable: false,
    message: 'agent added',
    steps: [
      { step: 'validate', status: 'succeeded', message: 'request validated' },
      { step: 'update_roster', status: 'succeeded', message: 'team roster updated' },
    ],
  }
}

export function buildMockLiveTeamStatus(teamName, overrides = {}) {
  const lead = overrides.lead ?? {}
  return {
    teamName,
    leadName: 'team-lead',
    members: [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: lead.cliTool ?? 'claude',
        model: lead.model ?? 'opus',
        roleId: lead.roleId ?? 'claude-orchestrator',
        roleName: lead.roleName ?? 'Claude Orchestrator',
        focusArea: lead.focusArea ?? 'Team sequencing and escalation',
        contextSummary: lead.contextSummary ?? 'Keeps the full delivery plan and blocker state in view.',
        behaviorSummary: lead.behaviorSummary ?? 'Coordinates specialists and escalates blockers.',
        projectId: 'proj-core',
        isCrossProject: false,
        projectLabel: '',
        description: lead.description ?? 'Own orchestration',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'codex',
        model: 'gpt-5.3',
        roleId: 'codex-developer',
        roleName: 'Codex Developer',
        focusArea: 'Scoped implementation',
        contextSummary: 'Owns code changes, tests, and debugging within assigned scope.',
        behaviorSummary: 'Implements narrowly and escalates blockers instead of broadening scope.',
        projectId: 'proj-web',
        isCrossProject: true,
        projectLabel: 'proj-web',
        description: 'UI implementation',
        sessionStatus: 'idle',
        paneId: '%2',
      },
    ],
  }
}

export function buildMockProjectMeshSnapshot(projectPath, overrides = {}) {
  const lead = overrides.lead ?? {}
  return {
    meshAvailable: true,
    tmuxAvailable: true,
    teamName: 'mock-team',
    teamRuntimeState: 'active',
    teamStatus: {
      leadName: 'team-lead',
      members: [
        {
          name: 'team-lead',
          role: 'lead',
          cliTool: lead.cliTool ?? 'claude',
          roleId: lead.roleId ?? 'claude-orchestrator',
          roleName: lead.roleName ?? 'Claude Orchestrator',
          focusArea: lead.focusArea ?? 'Team sequencing and escalation',
          contextSummary: lead.contextSummary ?? 'Keeps the full delivery plan and blocker state in view.',
          behaviorSummary: lead.behaviorSummary ?? 'Coordinates specialists and escalates blockers.',
          projectId: projectPath,
          isCrossProject: false,
          projectLabel: '',
          description: lead.description ?? 'Own orchestration',
          sessionStatus: 'active',
          paneId: '%1',
        },
      ],
    },
    warnings: [],
  }
}

export function buildMockCompactionAudit(teamName, overrides = {}) {
  return {
    teamName,
    entries: overrides.entries ?? [
      {
        memberName: 'frontend-dev',
        tool: 'codex',
        lastSessionId: '019ccdb2-09d5-7ff0-b5b6-72b7178c7dbf',
        lastCompactionTimestamp: '2026-03-08T14:46:41.037Z',
        lastDeliveryResult: 'injected',
      },
      {
        memberName: 'team-lead',
        tool: 'claude',
        lastSessionId: 'claude-session-1',
        lastCompactionTimestamp: '2026-03-08T14:22:10.000Z',
        lastDeliveryResult: 'skipped',
      },
    ],
  }
}

function buildMockResumeTeamReport(teamName) {
  return {
    teamName,
    resumed: true,
    totalMembers: 2,
    resumedMembers: ['team-lead', 'frontend-dev'],
    failedMembers: [],
    warnings: [],
    startedTeamDaemon: false,
    teamDaemonWarning: null,
  }
}
