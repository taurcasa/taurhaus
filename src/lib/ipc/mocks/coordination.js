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

export function buildMockLiveTeamStatus(teamName) {
  return {
    teamName,
    leadName: 'team-lead',
    members: [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: 'claude',
        model: 'opus',
        projectId: 'proj-core',
        description: 'Own orchestration',
        sessionStatus: 'active',
        paneId: '%1',
      },
      {
        name: 'frontend-dev',
        role: 'member',
        cliTool: 'codex',
        model: 'gpt-5.3',
        projectId: 'proj-web',
        description: 'UI implementation',
        sessionStatus: 'idle',
        paneId: '%2',
      },
    ],
  }
}
