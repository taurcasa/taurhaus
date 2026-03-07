import { beforeAll, beforeEach, afterAll, afterEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

function createMockComponent(name, renderContent) {
  return function MockComponent(target, props = {}) {
    let currentProps = props || {}
    const root = document.createElement('div')
    root.setAttribute('data-testid', `mock-${name}`)

    function update() {
      root.innerHTML = ''
      renderContent(root, currentProps)
    }

    update()

    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(root)
    } else {
      target.parentNode.insertBefore(root, target)
    }

    return {
      $set(nextProps) {
        currentProps = { ...currentProps, ...(nextProps || {}) }
        update()
      },
      $destroy() {
        root.remove()
      },
    }
  }
}

vi.mock('./lib/ipc.js', () => ({
  listProjects: vi.fn(),
  getProject: vi.fn(),
  getRecentCommits: vi.fn(),
  getAllCommits: vi.fn(),
  getReadme: vi.fn(),
  getLatestSession: vi.fn(),
  listSessions: vi.fn(),
  getRelationships: vi.fn(),
  dismissRelationship: vi.fn(),
  isTauri: vi.fn(() => false),
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
  coordinationGetProjectMeshSnapshot: vi.fn(),
  coordinationGetLiveTeamStatus: vi.fn(),
  coordinationInitializeTeam: vi.fn(),
  coordinationPreflightCheck: vi.fn(),
  coordinationRemoveMember: vi.fn(),
  coordinationResumeTeam: vi.fn(),
  coordinationResumeMember: vi.fn(),
  getRoleTemplate: vi.fn(),
  getTeamPreset: vi.fn(),
  installMesh: vi.fn(),
  listRoleTemplates: vi.fn(),
  listTeamPresets: vi.fn(),
  onCoordinationStepProgress: vi.fn(),
  upsertRoleTemplate: vi.fn(),
}))

vi.mock('./lib/sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  applyDaemonSessionUpdate: vi.fn(),
  hydrateFromBackend: vi.fn(),
  startPolling: vi.fn(),
  stopPolling: vi.fn(),
}))

vi.mock('./lib/projectSelection.js', () => ({
  loadProjectSelectionData: vi.fn(),
}))

vi.mock('./lib/shell/themePreferences.js', () => ({
  loadThemePreferences: vi.fn(),
  persistDarkModePreference: vi.fn(),
}))

vi.mock('./lib/context/ProjectContext.js', () => ({
  setProjectContext: vi.fn((value) => value),
}))

vi.mock('./lib/context/SessionContext.js', () => ({
  setSessionContext: vi.fn((value) => value),
}))

vi.mock('./lib/assetCache.js', () => ({
  invalidateByPrefix: vi.fn(),
}))

vi.mock('./lib/fileChange.js', () => ({
  anyPathMatches: vi.fn(() => false),
}))

vi.mock('./lib/Sidebar.svelte', () => ({
  default: createMockComponent('sidebar', () => {}),
}))

vi.mock('./lib/OverviewTab.svelte', () => ({
  default: createMockComponent('overview', (root, props) => {
    const projectId = props?.data?.selectedProject?.id ?? 'none'
    root.innerHTML = `<div data-testid="overview-project-id">${projectId}</div>`
  }),
}))

vi.mock('./lib/FilesTab.svelte', () => ({
  default: createMockComponent('files', () => {}),
}))

vi.mock('./lib/TaskBoard.svelte', () => ({
  default: createMockComponent('tasks', () => {}),
}))

vi.mock('./lib/GitTab.svelte', () => ({
  default: createMockComponent('git', () => {}),
}))

vi.mock('./lib/SearchOverlay.svelte', () => ({
  default: createMockComponent('search', () => {}),
}))

vi.mock('./lib/Settings.svelte', () => ({
  default: createMockComponent('settings', () => {}),
}))

vi.mock('./lib/AddProjectModal.svelte', () => ({
  default: createMockComponent('add-project', () => {}),
}))

vi.mock('./lib/FirstRunWizard.svelte', () => ({
  default: createMockComponent('first-run', () => {}),
}))

const ipc = await import('./lib/ipc.js')
const { loadProjectSelectionData } = await import('./lib/projectSelection.js')
const { loadThemePreferences } = await import('./lib/shell/themePreferences.js')
const { setProjectContext } = await import('./lib/context/ProjectContext.js')

import Shell from './Shell.svelte'

function buildLiveTeamStatus(projectLabel = 'taurhaus') {
  return {
    teamName: `${projectLabel}-team`,
    leadName: 'team-lead',
    members: [
      {
        name: 'team-lead',
        role: 'lead',
        cliTool: 'claude',
        model: 'opus',
        roleId: 'claude-orchestrator',
        roleName: 'Claude Orchestrator',
        focusArea: 'Team sequencing',
        contextSummary: 'Maintains delivery context.',
        behaviorSummary: 'Coordinates specialists.',
        projectId: `${projectLabel}-core`,
        sessionStatus: 'active',
        paneId: '%1',
      },
      ...Array.from({ length: 6 }, (_, index) => ({
        name: `developer-${index + 1}`,
        role: 'member',
        cliTool: index % 3 === 0 ? 'codex' : (index % 3 === 1 ? 'claude' : 'gemini'),
        model: 'gpt-5.4 high',
        roleId: `role-${index + 1}`,
        roleName: `Role ${index + 1}`,
        focusArea: `Focus ${index + 1}`,
        contextSummary: `Context ${index + 1}`,
        behaviorSummary: `Behavior ${index + 1}`,
        projectId: `${projectLabel}-${index + 1}`,
        description: `Agent ${index + 1}`,
        sessionStatus: index % 2 === 0 ? 'active' : 'idle',
        paneId: `%${index + 2}`,
      })),
    ],
  }
}

function buildRuntimeSnapshot(projectLabel = 'taurhaus') {
  const liveStatus = buildLiveTeamStatus(projectLabel)
  return {
    meshAvailable: true,
    tmuxAvailable: true,
    teamName: liveStatus.teamName,
    teamRuntimeState: 'active',
    warnings: [],
    teamStatus: {
      leadName: liveStatus.leadName,
      members: liveStatus.members.map(({ model, ...member }) => member),
    },
  }
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function renderShellWithDelayedSelectionData() {
  loadProjectSelectionData.mockImplementation(async (projectId) => {
    await delay(40)
    return {
      detail: {
        ok: true,
        section: 'Project details',
        value: projectId === 'proj-2'
          ? { id: 'proj-2', path: '/projects/mein-ponyhof', name: 'mein-ponyhof' }
          : { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' },
      },
      commits: { ok: true, section: 'Recent commits', value: [] },
      latest: { ok: true, section: 'Latest session', value: null },
      sessionList: { ok: true, section: 'Session history', value: [] },
      readme: { ok: true, section: 'README', value: null },
      rels: { ok: true, section: 'Relationships', value: [] },
    }
  })

  render(Shell)

  await waitFor(() => {
    expect(setProjectContext.mock.calls.at(-1)?.[0]?.selectedProject?.id).toBe('proj-1')
    expect(screen.getByTestId('tab-mesh')).toBeInTheDocument()
  })

  return {
    projectContext: setProjectContext.mock.calls.at(-1)?.[0],
    projectOne: {
      id: 'proj-1',
      name: 'taurhaus',
      path: '/projects/taurhaus',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    },
    projectTwo: {
      id: 'proj-2',
      name: 'mein-ponyhof',
      path: '/projects/mein-ponyhof',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    },
  }
}

let previousResizeObserver

beforeAll(() => {
  previousResizeObserver = globalThis.ResizeObserver
  globalThis.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
})

afterAll(() => {
  if (previousResizeObserver) {
    globalThis.ResizeObserver = previousResizeObserver
    return
  }
  delete globalThis.ResizeObserver
})

describe('Shell mesh switch performance', () => {
  let measurements

  beforeEach(() => {
    vi.clearAllMocks()
    measurements = []
    globalThis.__TAURHAUS_PROJECT_SWITCH_MEASURE__ = (measurement) => {
      measurements.push(measurement)
    }

    ipc.listProjects.mockResolvedValue([
      {
        id: 'proj-1',
        name: 'taurhaus',
        path: '/projects/taurhaus',
        activityState: 'active',
        branch: 'main',
        isDirty: false,
      },
      {
        id: 'proj-2',
        name: 'mein-ponyhof',
        path: '/projects/mein-ponyhof',
        activityState: 'active',
        branch: 'main',
        isDirty: false,
      },
    ])
    ipc.isFirstRun.mockResolvedValue(false)
    ipc.getSettings.mockResolvedValue({ dark_mode: false, code_theme: null })
    ipc.getDaemonStatus.mockResolvedValue('connected')
    ipc.checkDaemonInstallStatus.mockResolvedValue({ installed: true, needs_update: false })
    ipc.getPlatform.mockResolvedValue('linux')
    ipc.navigateToSession.mockResolvedValue(undefined)
    ipc.listClaudeSessions.mockResolvedValue([])

    ipc.checkMeshInstallStatus.mockResolvedValue({
      installed: true,
      version: '0.1.0',
      bundled_version: '0.1.0',
      needs_update: false,
      environment_available: true,
      error: null,
    })
    ipc.coordinationPreflightCheck.mockResolvedValue({
      blockingErrors: [],
      agentWarnings: [],
    })
    ipc.installMesh.mockResolvedValue({
      success: true,
      message: 'Mesh installed successfully',
    })
    ipc.coordinationGetProjectMeshSnapshot.mockImplementation(async (projectPath) => {
      if (projectPath === '/projects/taurhaus') return buildRuntimeSnapshot('taurhaus')
      return {
        meshAvailable: true,
        tmuxAvailable: true,
        teamName: null,
        teamRuntimeState: 'none',
        teamStatus: null,
        warnings: [],
      }
    })
    ipc.coordinationGetLiveTeamStatus.mockResolvedValue(buildLiveTeamStatus('taurhaus'))
    ipc.coordinationInitializeTeam.mockResolvedValue({ teamName: 'taurhaus-team', steps: [] })
    ipc.coordinationAddAgent.mockResolvedValue({ teamName: 'taurhaus-team', memberName: 'backend-dev', steps: [] })
    ipc.coordinationDisbandTeam.mockResolvedValue({ teamName: 'taurhaus-team', disbanded: true, alreadyDisbanded: false })
    ipc.coordinationRemoveMember.mockResolvedValue({ teamName: 'taurhaus-team', memberName: 'developer-1', removed: true, steps: [] })
    ipc.coordinationResumeTeam.mockResolvedValue({
      teamName: 'taurhaus-team',
      resumed: true,
      totalMembers: 7,
      resumedMembers: ['team-lead'],
      failedMembers: [],
      warnings: [],
      startedTeamDaemon: false,
      teamDaemonWarning: null,
    })
    ipc.coordinationResumeMember.mockResolvedValue({
      teamName: 'taurhaus-team',
      memberName: 'developer-1',
      resumed: true,
      steps: [],
      warnings: [],
    })
    ipc.listRoleTemplates.mockResolvedValue([])
    ipc.listTeamPresets.mockResolvedValue([])
    ipc.getRoleTemplate.mockResolvedValue(null)
    ipc.getTeamPreset.mockResolvedValue(null)
    ipc.composeTeam.mockResolvedValue({ roster: [] })
    ipc.onCoordinationStepProgress.mockReturnValue(() => {})
    ipc.upsertRoleTemplate.mockResolvedValue({ ok: true })

    loadThemePreferences.mockResolvedValue({
      codeThemeLight: 'github-light',
      codeThemeDark: 'github-dark-dimmed',
      darkMode: false,
    })

    loadProjectSelectionData.mockImplementation(async (projectId) => ({
      detail: {
        ok: true,
        section: 'Project details',
        value: projectId === 'proj-2'
          ? { id: 'proj-2', path: '/projects/mein-ponyhof', name: 'mein-ponyhof' }
          : { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' },
      },
      commits: { ok: true, section: 'Recent commits', value: [] },
      latest: { ok: true, section: 'Latest session', value: null },
      sessionList: { ok: true, section: 'Session history', value: [] },
      readme: { ok: true, section: 'README', value: null },
      rels: { ok: true, section: 'Relationships', value: [] },
    }))
  })

  afterEach(() => {
    delete globalThis.__TAURHAUS_PROJECT_SWITCH_MEASURE__
  })

  it('switches visible shell state immediately when leaving mesh', async () => {
    const { projectContext, projectTwo } = await renderShellWithDelayedSelectionData()

    await fireEvent.click(screen.getByTestId('tab-mesh'))
    await waitFor(() => {
      expect(screen.getByTestId('mesh-mode-runtime')).toBeInTheDocument()
    })

    measurements = []
    await projectContext.selectProject(projectTwo)
    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-2')
      expect(measurements).toHaveLength(1)
    })
    const meshAway = measurements[0]
    expect(meshAway.fromTab).toBe('mesh')
    expect(meshAway.visibleMs).toBeLessThan(20)
    expect(meshAway.totalMs).toBeGreaterThanOrEqual(35)

    console.info('[perf-test] delayed mesh away', { measurement: meshAway })
  })

  it('switches visible shell state immediately when leaving overview', async () => {
    const { projectContext, projectTwo } = await renderShellWithDelayedSelectionData()

    measurements = []
    await projectContext.selectProject(projectTwo)
    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-2')
      expect(measurements).toHaveLength(1)
    })
    const overviewAway = measurements[0]

    expect(overviewAway.fromTab).toBe('overview')
    expect(overviewAway.visibleMs).toBeLessThan(20)
    expect(overviewAway.totalMs).toBeGreaterThanOrEqual(35)

    console.info('[perf-test] delayed overview away', { measurement: overviewAway })
  })
})
