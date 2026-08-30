import { vi } from 'vitest'

// Imported from the submodule, so it is the real implementation: the `vi.mock`
// below replaces `../../lib/ipc.js`, not the modules behind it.
import { buildFrontendFallbackTerminalContract as actualFallbackTerminalContract } from '../../lib/ipc/system.js'

function createMockMap() {
  return {
    listProjects: vi.fn(),
    getProject: vi.fn(),
    createProject: vi.fn(),
    getRecentCommits: vi.fn(),
    getAllCommits: vi.fn(),
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
    listCliSessionSnapshot: vi.fn(),
    getForegroundProject: vi.fn(),
    resolveLaunchAccount: vi.fn(),
    recordSessionActivity: vi.fn(),
    getRelationships: vi.fn(),
    dismissRelationship: vi.fn(),
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
    launchCliSession: vi.fn(),
    navigateToSession: vi.fn(),
    stopClaudeSession: vi.fn(),
    removeProject: vi.fn(),
    getRemoteUrl: vi.fn(),
    checkPathType: vi.fn(),
    openExternalUrl: vi.fn(),
    getPlatform: vi.fn(),
    listClaudeSessions: vi.fn(),
    listAccounts: vi.fn(),
    refreshAccountsUsage: vi.fn(),
    setProjectAccount: vi.fn(),
    buildFrontendFallbackTerminalContract: vi.fn(),
    search: vi.fn(),
    getProjectTasks: vi.fn(),
    listWorkflowRuns: vi.fn(),
    getWorkflowRun: vi.fn(),
    workflowLedgerRow: vi.fn(),
    getTaskDetail: vi.fn(),
    getArchivedSessions: vi.fn(),
    getAllCommits: vi.fn(),
    getCommitFiles: vi.fn(),
    getCommitDiff: vi.fn(),
    getCommitsInRange: vi.fn(),
    checkMeshInstallStatus: vi.fn(),
    coordinationGetProjectMeshSnapshot: vi.fn(),
    composeTeam: vi.fn(),
    coordinationAddAgent: vi.fn(),
    coordinationDisbandTeam: vi.fn(),
    coordinationGetLiveTeamStatus: vi.fn(),
    coordinationInitializeTeam: vi.fn(),
    coordinationListTeams: vi.fn(),
    coordinationPreflightCheck: vi.fn(),
    coordinationRemoveMember: vi.fn(),
    coordinationResumeMember: vi.fn(),
    coordinationResumeTeam: vi.fn(),
    getRoleTemplate: vi.fn(),
    upsertRoleTemplate: vi.fn(),
    deleteRoleTemplate: vi.fn(),
    getTeamPreset: vi.fn(),
    upsertTeamPreset: vi.fn(),
    deleteTeamPreset: vi.fn(),
    importRoleFromFile: vi.fn(),
    exportRoleToFile: vi.fn(),
    exportAgentDefinitions: vi.fn(),
    getTemplateStorageStatus: vi.fn(),
    getTemplateHistory: vi.fn(),
    getTemplateDiff: vi.fn(),
    revertTemplateVersion: vi.fn(),
    registerProjectsBatch: vi.fn(),
    installMesh: vi.fn(),
    listRoleTemplates: vi.fn(),
    listTeamPresets: vi.fn(),
    onCoordinationStepProgress: vi.fn(),
    onCoordinationResumeTeamProgress: vi.fn(),
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
  visualIpcMocks.createProject.mockResolvedValue(null)
  visualIpcMocks.getRecentCommits.mockResolvedValue([])
  visualIpcMocks.getAllCommits.mockResolvedValue([])
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
  visualIpcMocks.listCliSessionSnapshot.mockResolvedValue({ sessions: [], freshness: 'fresh' })
  visualIpcMocks.getForegroundProject.mockResolvedValue(null)
  visualIpcMocks.resolveLaunchAccount.mockResolvedValue({
    accountId: null,
    email: null,
    source: 'default_config_dir',
    needsChoice: false,
  })
  visualIpcMocks.recordSessionActivity.mockResolvedValue(undefined)
  visualIpcMocks.getRelationships.mockResolvedValue([])
  visualIpcMocks.dismissRelationship.mockResolvedValue(undefined)
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
  visualIpcMocks.launchCliSession.mockResolvedValue(undefined)
  visualIpcMocks.listAccounts.mockResolvedValue({
    accounts: [],
    source: 'native',
    degraded: false,
    error: null,
  })
  visualIpcMocks.refreshAccountsUsage.mockResolvedValue(false)
  visualIpcMocks.setProjectAccount.mockResolvedValue(undefined)
  // A pure helper, not an IPC call: the visual lane wants the real contract.
  visualIpcMocks.buildFrontendFallbackTerminalContract.mockImplementation(
    (platform = 'linux') => actualFallbackTerminalContract(platform),
  )
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
  visualIpcMocks.listWorkflowRuns.mockResolvedValue([])
  visualIpcMocks.getWorkflowRun.mockResolvedValue(null)
  visualIpcMocks.workflowLedgerRow.mockResolvedValue(null)
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
  visualIpcMocks.coordinationGetProjectMeshSnapshot.mockResolvedValue(null)
  visualIpcMocks.composeTeam.mockResolvedValue({ roster: [], warnings: [], validationErrors: [] })
  visualIpcMocks.coordinationAddAgent.mockResolvedValue(undefined)
  visualIpcMocks.coordinationDisbandTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationGetLiveTeamStatus.mockResolvedValue({ teamName: '', leadName: '', members: [] })
  visualIpcMocks.coordinationInitializeTeam.mockResolvedValue(undefined)
  visualIpcMocks.coordinationListTeams.mockResolvedValue([])
  visualIpcMocks.coordinationPreflightCheck.mockResolvedValue({ canInitialize: true, blockingErrors: [], agentWarnings: [] })
  visualIpcMocks.coordinationRemoveMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationResumeMember.mockResolvedValue(undefined)
  visualIpcMocks.coordinationResumeTeam.mockResolvedValue(undefined)
  visualIpcMocks.getRoleTemplate.mockResolvedValue(null)
  visualIpcMocks.upsertRoleTemplate.mockResolvedValue(undefined)
  visualIpcMocks.deleteRoleTemplate.mockResolvedValue(undefined)
  visualIpcMocks.getTeamPreset.mockResolvedValue(null)
  visualIpcMocks.upsertTeamPreset.mockResolvedValue(undefined)
  visualIpcMocks.deleteTeamPreset.mockResolvedValue(undefined)
  visualIpcMocks.importRoleFromFile.mockResolvedValue({ success: true, role: null, conflict: null })
  visualIpcMocks.exportRoleToFile.mockResolvedValue({ targetFormat: 'claude_agent', fileContent: '', lossyFields: [] })
  visualIpcMocks.exportAgentDefinitions.mockResolvedValue({ written: [], removed: [], skipped: [] })
  visualIpcMocks.getTemplateStorageStatus.mockResolvedValue({ dirty: false, head: null, branch: null })
  visualIpcMocks.getTemplateHistory.mockResolvedValue({ commits: [], cursor: null, has_more: false })
  visualIpcMocks.getTemplateDiff.mockResolvedValue(null)
  visualIpcMocks.revertTemplateVersion.mockResolvedValue(undefined)
  visualIpcMocks.registerProjectsBatch.mockResolvedValue([])
  visualIpcMocks.installMesh.mockResolvedValue({ success: true, message: 'Mesh installed successfully.' })
  visualIpcMocks.listRoleTemplates.mockResolvedValue([])
  visualIpcMocks.listTeamPresets.mockResolvedValue([])
  visualIpcMocks.onCoordinationStepProgress.mockResolvedValue(() => {})
  visualIpcMocks.onCoordinationResumeTeamProgress.mockResolvedValue(() => {})

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
