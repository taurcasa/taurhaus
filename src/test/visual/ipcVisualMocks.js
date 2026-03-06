import { vi } from 'vitest'

function createMockMap() {
  return {
    listProjects: vi.fn(),
    getProject: vi.fn(),
    getRecentCommits: vi.fn(),
    getAllCommits: vi.fn(),
    getReadme: vi.fn(),
    getLatestSession: vi.fn(),
    listSessions: vi.fn(),
    getRelationships: vi.fn(),
    dismissRelationship: vi.fn(),
    isTauri: vi.fn(),
    isFirstRun: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    getDaemonStatus: vi.fn(),
    checkDaemonInstallStatus: vi.fn(),
    installDaemon: vi.fn(),
    launchClaudeSession: vi.fn(),
    navigateToSession: vi.fn(),
    getRemoteUrl: vi.fn(),
    checkPathType: vi.fn(),
    openExternalUrl: vi.fn(),
    getPlatform: vi.fn(),
    listClaudeSessions: vi.fn(),
    checkMeshInstallStatus: vi.fn(),
    composeTeam: vi.fn(),
    coordinationAddAgent: vi.fn(),
    coordinationDisbandTeam: vi.fn(),
    coordinationGetLiveTeamStatus: vi.fn(),
    coordinationInitializeTeam: vi.fn(),
    coordinationListTeams: vi.fn(),
    coordinationPreflightCheck: vi.fn(),
    coordinationRemoveMember: vi.fn(),
    coordinationResumeMember: vi.fn(),
    getRoleTemplate: vi.fn(),
    getTeamPreset: vi.fn(),
    installMesh: vi.fn(),
    listRoleTemplates: vi.fn(),
    listTeamPresets: vi.fn(),
    onCoordinationStepProgress: vi.fn(),
  }
}

export const visualIpcMocks = createMockMap()

vi.mock('../../lib/ipc.js', () => visualIpcMocks)

export function resetVisualIpcMocks(overrides = {}) {
  for (const mock of Object.values(visualIpcMocks)) {
    mock.mockReset()
  }

  visualIpcMocks.listProjects.mockResolvedValue([])
  visualIpcMocks.getProject.mockResolvedValue(null)
  visualIpcMocks.getRecentCommits.mockResolvedValue([])
  visualIpcMocks.getAllCommits.mockResolvedValue([])
  visualIpcMocks.getReadme.mockResolvedValue(null)
  visualIpcMocks.getLatestSession.mockResolvedValue(null)
  visualIpcMocks.listSessions.mockResolvedValue([])
  visualIpcMocks.getRelationships.mockResolvedValue([])
  visualIpcMocks.dismissRelationship.mockResolvedValue(undefined)
  visualIpcMocks.isTauri.mockReturnValue(false)
  visualIpcMocks.isFirstRun.mockResolvedValue(false)
  visualIpcMocks.getSettings.mockResolvedValue({ dark_mode: false, code_theme: null })
  visualIpcMocks.updateSettings.mockResolvedValue(undefined)
  visualIpcMocks.getDaemonStatus.mockResolvedValue('connected')
  visualIpcMocks.checkDaemonInstallStatus.mockResolvedValue({ installed: true, needs_update: false })
  visualIpcMocks.installDaemon.mockResolvedValue(undefined)
  visualIpcMocks.launchClaudeSession.mockResolvedValue(undefined)
  visualIpcMocks.navigateToSession.mockResolvedValue(undefined)
  visualIpcMocks.getRemoteUrl.mockResolvedValue(null)
  visualIpcMocks.checkPathType.mockResolvedValue('not_found')
  visualIpcMocks.openExternalUrl.mockResolvedValue(undefined)
  visualIpcMocks.getPlatform.mockResolvedValue('linux')
  visualIpcMocks.listClaudeSessions.mockResolvedValue([])
  visualIpcMocks.checkMeshInstallStatus.mockResolvedValue({
    installed: true,
    needs_update: false,
    environment_available: true,
    error: null,
  })
  visualIpcMocks.composeTeam.mockResolvedValue({ roster: [], warnings: [], validationErrors: [] })
  visualIpcMocks.coordinationAddAgent.mockResolvedValue(undefined)
  visualIpcMocks.coordinationDisbandTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationGetLiveTeamStatus.mockResolvedValue({ teamName: '', leadName: '', members: [] })
  visualIpcMocks.coordinationInitializeTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationListTeams.mockResolvedValue([])
  visualIpcMocks.coordinationPreflightCheck.mockResolvedValue({ canInitialize: true, blockingErrors: [], agentWarnings: [] })
  visualIpcMocks.coordinationRemoveMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationResumeMember.mockResolvedValue(undefined)
  visualIpcMocks.getRoleTemplate.mockResolvedValue(null)
  visualIpcMocks.getTeamPreset.mockResolvedValue(null)
  visualIpcMocks.installMesh.mockResolvedValue({ success: true, message: 'Mesh installed successfully.' })
  visualIpcMocks.listRoleTemplates.mockResolvedValue([])
  visualIpcMocks.listTeamPresets.mockResolvedValue([])
  visualIpcMocks.onCoordinationStepProgress.mockResolvedValue(() => {})

  for (const [name, value] of Object.entries(overrides)) {
    const mock = visualIpcMocks[name]
    if (!mock) continue
    if (typeof value === 'function') {
      mock.mockImplementation(value)
      continue
    }
    mock.mockResolvedValue(value)
  }

  return visualIpcMocks
}
