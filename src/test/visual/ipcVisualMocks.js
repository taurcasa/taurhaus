import { vi } from 'vitest'

function createMockMap() {
  return {
    listProjects: vi.fn(),
    getProject: vi.fn(),
    registerProject: vi.fn(),
    createProject: vi.fn(),
    updateProject: vi.fn(),
    getRecentCommits: vi.fn(),
    getAllCommits: vi.fn(),
    getGitStatus: vi.fn(),
    getReadme: vi.fn(),
    getFileTree: vi.fn(),
    readFile: vi.fn(),
    readProjectAsset: vi.fn(),
    scanDirectory: vi.fn(),
    listDirectory: vi.fn(),
    getSystemRoots: vi.fn(),
    validateProjectPath: vi.fn(),
    getLatestSession: vi.fn(),
    listSessions: vi.fn(),
    getSession: vi.fn(),
    getProjectActivity: vi.fn(),
    recordSessionActivity: vi.fn(),
    getRelationships: vi.fn(),
    dismissRelationship: vi.fn(),
    createRelationship: vi.fn(),
    removeRelationship: vi.fn(),
    isTauri: vi.fn(),
    isFirstRun: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    getIndexStatus: vi.fn(),
    rebuildIndex: vi.fn(),
    getDaemonStatus: vi.fn(),
    startDaemon: vi.fn(),
    checkDaemonInstallStatus: vi.fn(),
    installDaemon: vi.fn(),
    launchClaudeSession: vi.fn(),
    navigateToSession: vi.fn(),
    stopClaudeSession: vi.fn(),
    removeProject: vi.fn(),
    getRemoteUrl: vi.fn(),
    checkPathType: vi.fn(),
    openExternalUrl: vi.fn(),
    getPlatform: vi.fn(),
    listClaudeSessions: vi.fn(),
    listClaudeAccounts: vi.fn(),
    setProjectClaudeAccount: vi.fn(),
    search: vi.fn(),
    getProjectTasks: vi.fn(),
    getTaskDetail: vi.fn(),
    getArchivedSessions: vi.fn(),
    getAllCommits: vi.fn(),
    getCommitFiles: vi.fn(),
    getCommitDiff: vi.fn(),
    getCommitsInRange: vi.fn(),
    checkMeshInstallStatus: vi.fn(),
    coordinationCreateTeam: vi.fn(),
    coordinationGetFeatureAvailability: vi.fn(),
    coordinationGetProjectMeshSnapshot: vi.fn(),
    coordinationGetTeamStatus: vi.fn(),
    composeTeam: vi.fn(),
    coordinationAddMember: vi.fn(),
    coordinationAddAgent: vi.fn(),
    coordinationDisbandTeam: vi.fn(),
    coordinationGetLiveTeamStatus: vi.fn(),
    coordinationInitializeTeam: vi.fn(),
    coordinationListTeams: vi.fn(),
    coordinationPreflightCheck: vi.fn(),
    coordinationReonboard: vi.fn(),
    coordinationRemoveMember: vi.fn(),
    coordinationResumeMember: vi.fn(),
    coordinationResumeTeam: vi.fn(),
    getRoleTemplate: vi.fn(),
    upsertRoleTemplate: vi.fn(),
    deleteRoleTemplate: vi.fn(),
    getTeamPreset: vi.fn(),
    upsertTeamPreset: vi.fn(),
    deleteTeamPreset: vi.fn(),
    getTemplateStorageStatus: vi.fn(),
    getTemplateHistory: vi.fn(),
    getTemplateDiff: vi.fn(),
    revertTemplateVersion: vi.fn(),
    registerProjectsBatch: vi.fn(),
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
  visualIpcMocks.registerProject.mockResolvedValue(null)
  visualIpcMocks.createProject.mockResolvedValue(null)
  visualIpcMocks.updateProject.mockResolvedValue(null)
  visualIpcMocks.getRecentCommits.mockResolvedValue([])
  visualIpcMocks.getAllCommits.mockResolvedValue([])
  visualIpcMocks.getGitStatus.mockResolvedValue({ modified: [], untracked: [], staged: [] })
  visualIpcMocks.getReadme.mockResolvedValue(null)
  visualIpcMocks.getFileTree.mockResolvedValue([])
  visualIpcMocks.readFile.mockResolvedValue(null)
  visualIpcMocks.readProjectAsset.mockResolvedValue(null)
  visualIpcMocks.scanDirectory.mockResolvedValue([])
  visualIpcMocks.listDirectory.mockResolvedValue([])
  visualIpcMocks.getSystemRoots.mockResolvedValue([])
  visualIpcMocks.validateProjectPath.mockResolvedValue({ valid: true, message: null })
  visualIpcMocks.getLatestSession.mockResolvedValue(null)
  visualIpcMocks.listSessions.mockResolvedValue([])
  visualIpcMocks.getSession.mockResolvedValue(null)
  visualIpcMocks.getProjectActivity.mockResolvedValue(null)
  visualIpcMocks.recordSessionActivity.mockResolvedValue(undefined)
  visualIpcMocks.getRelationships.mockResolvedValue([])
  visualIpcMocks.dismissRelationship.mockResolvedValue(undefined)
  visualIpcMocks.createRelationship.mockResolvedValue(null)
  visualIpcMocks.removeRelationship.mockResolvedValue(undefined)
  visualIpcMocks.isTauri.mockReturnValue(false)
  visualIpcMocks.isFirstRun.mockResolvedValue(false)
  visualIpcMocks.getSettings.mockResolvedValue({ dark_mode: false, code_theme: null })
  visualIpcMocks.updateSettings.mockResolvedValue(undefined)
  visualIpcMocks.getIndexStatus.mockResolvedValue({ ready: true, indexing: false, documents: 0 })
  visualIpcMocks.rebuildIndex.mockResolvedValue(undefined)
  visualIpcMocks.getDaemonStatus.mockResolvedValue('connected')
  visualIpcMocks.startDaemon.mockResolvedValue(undefined)
  visualIpcMocks.checkDaemonInstallStatus.mockResolvedValue({ installed: true, needs_update: false })
  visualIpcMocks.installDaemon.mockResolvedValue(undefined)
  visualIpcMocks.launchClaudeSession.mockResolvedValue(undefined)
  visualIpcMocks.listClaudeAccounts.mockResolvedValue([])
  visualIpcMocks.setProjectClaudeAccount.mockResolvedValue(undefined)
  visualIpcMocks.navigateToSession.mockResolvedValue(undefined)
  visualIpcMocks.stopClaudeSession.mockResolvedValue(undefined)
  visualIpcMocks.removeProject.mockResolvedValue(undefined)
  visualIpcMocks.getRemoteUrl.mockResolvedValue(null)
  visualIpcMocks.checkPathType.mockResolvedValue('not_found')
  visualIpcMocks.openExternalUrl.mockResolvedValue(undefined)
  visualIpcMocks.getPlatform.mockResolvedValue('linux')
  visualIpcMocks.listClaudeSessions.mockResolvedValue([])
  visualIpcMocks.search.mockResolvedValue([])
  visualIpcMocks.getProjectTasks.mockResolvedValue({ tasks: [], errors: [] })
  visualIpcMocks.getTaskDetail.mockResolvedValue(null)
  visualIpcMocks.getArchivedSessions.mockResolvedValue([])
  visualIpcMocks.getAllCommits.mockResolvedValue([])
  visualIpcMocks.getCommitFiles.mockResolvedValue([])
  visualIpcMocks.getCommitDiff.mockResolvedValue([])
  visualIpcMocks.getCommitsInRange.mockResolvedValue({ commits: [], files: [], truncated: false, total_count: null })
  visualIpcMocks.checkMeshInstallStatus.mockResolvedValue({
    installed: true,
    needs_update: false,
    environment_available: true,
    error: null,
  })
  visualIpcMocks.coordinationCreateTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationGetFeatureAvailability.mockResolvedValue({ available: true, reason: null })
  visualIpcMocks.coordinationGetProjectMeshSnapshot.mockResolvedValue(null)
  visualIpcMocks.coordinationGetTeamStatus.mockResolvedValue(null)
  visualIpcMocks.composeTeam.mockResolvedValue({ roster: [], warnings: [], validationErrors: [] })
  visualIpcMocks.coordinationAddMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationAddAgent.mockResolvedValue(undefined)
  visualIpcMocks.coordinationDisbandTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationGetLiveTeamStatus.mockResolvedValue({ teamName: '', leadName: '', members: [] })
  visualIpcMocks.coordinationInitializeTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationListTeams.mockResolvedValue([])
  visualIpcMocks.coordinationPreflightCheck.mockResolvedValue({ canInitialize: true, blockingErrors: [], agentWarnings: [] })
  visualIpcMocks.coordinationReonboard.mockResolvedValue(undefined)
  visualIpcMocks.coordinationRemoveMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationResumeMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationResumeTeam.mockResolvedValue(undefined)
  visualIpcMocks.getRoleTemplate.mockResolvedValue(null)
  visualIpcMocks.upsertRoleTemplate.mockResolvedValue(undefined)
  visualIpcMocks.deleteRoleTemplate.mockResolvedValue(undefined)
  visualIpcMocks.getTeamPreset.mockResolvedValue(null)
  visualIpcMocks.upsertTeamPreset.mockResolvedValue(undefined)
  visualIpcMocks.deleteTeamPreset.mockResolvedValue(undefined)
  visualIpcMocks.getTemplateStorageStatus.mockResolvedValue({ dirty: false, head: null, branch: null })
  visualIpcMocks.getTemplateHistory.mockResolvedValue({ commits: [], cursor: null, has_more: false })
  visualIpcMocks.getTemplateDiff.mockResolvedValue(null)
  visualIpcMocks.revertTemplateVersion.mockResolvedValue(undefined)
  visualIpcMocks.registerProjectsBatch.mockResolvedValue([])
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
