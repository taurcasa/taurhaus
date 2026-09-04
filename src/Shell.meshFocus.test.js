import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

let meshTabMountCount = 0
let latestSidebarProps = {}
let mockSessionMap = new Map()

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

function lastTextByTestId(testId) {
  const matches = screen.getAllByTestId(testId)
  return matches.at(-1)?.textContent ?? ''
}

vi.mock('./lib/ipc.js', () => ({
  refreshAccountsUsage: vi.fn(() => Promise.resolve(true)),
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
  launchCliSession: vi.fn(),
  navigateToSession: vi.fn(),
  getForegroundProject: vi.fn(),
  getRemoteUrl: vi.fn(),
  checkPathType: vi.fn(),
  openExternalUrl: vi.fn(),
  getPlatform: vi.fn(),
  listClaudeSessions: vi.fn(),
  listAccounts: vi.fn(() =>
    Promise.resolve({ accounts: [], source: 'native', degraded: false, error: null })
  ),
  // Regression: 971d9643 added the app-side account reverse index, and Shell's
  // startup refresh made this integration mock part of that IPC contract.
  listAccountRelationships: vi.fn(() => Promise.resolve({ byAccount: {} })),
  setProjectAccount: vi.fn(() => Promise.resolve()),
  startDaemon: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

vi.mock('./lib/sessionStore.svelte.js', () => ({
  DEFAULT_TAURI_POLL_INTERVAL_MS: 5000,
  getSessionForProject: vi.fn(() => null),
  getSessions: vi.fn(() => mockSessionMap),
  applyDaemonSessionUpdate: vi.fn(),
  hydrateFromBackend: vi.fn(),
  markSessionPresenceStale: vi.fn(),
  startPolling: vi.fn(),
  stopPolling: vi.fn(),
}))

vi.mock('./lib/projectSelection.js', () => ({
  classifyProjectLoadResults: vi.fn((results, { deferRetryableIssues = false } = {}) => {
    const issues = results
      .filter((result) => !result.ok)
      .map((result) => ({
        section: result.section,
        message: result.message,
        retryableOnDaemonReconnect: Boolean(result.retryableOnDaemonReconnect),
      }))

    return {
      issues,
      pendingRetry:
        deferRetryableIssues && issues.some((issue) => issue.retryableOnDaemonReconnect),
      visibleIssues: deferRetryableIssues
        ? issues.filter((issue) => !issue.retryableOnDaemonReconnect)
        : issues,
    }
  }),
  loadProjectSelectionData: vi.fn(),
  loadDeferredProjectSelectionData: vi.fn(),
  prefetchProjectSelectionData: vi.fn(() => Promise.resolve(null)),
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
  default: createMockComponent('sidebar', (root, props) => {
    latestSidebarProps = props
    const foreground = document.createElement('div')
    foreground.setAttribute('data-testid', 'sidebar-foreground-project')
    foreground.textContent = props.foregroundProjectId ?? ''
    root.appendChild(foreground)

    const optimistic = document.createElement('button')
    optimistic.type = 'button'
    optimistic.textContent = 'Optimistic Foreground'
    optimistic.setAttribute('data-testid', 'sidebar-foreground-trigger')
    optimistic.onclick = () => props.onForegroundProjectChange?.('proj-2')
    root.appendChild(optimistic)
  }),
}))

vi.mock('./lib/OverviewTab.svelte', () => ({
  default: createMockComponent('overview', () => {}),
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

vi.mock('./lib/ProjectsTakeover.svelte', () => ({
  default: createMockComponent('projects-takeover', () => {}),
}))

vi.mock('./lib/FirstRunWizard.svelte', () => ({
  default: createMockComponent('first-run', () => {}),
}))

vi.mock('./lib/components/MeshTab.svelte', () => ({
  default: createMockComponent('mesh-tab', (root, props) => {
    if (!root.dataset.mountId) {
      meshTabMountCount += 1
      root.dataset.mountId = String(meshTabMountCount)
    }

    const mountId = document.createElement('div')
    mountId.setAttribute('data-testid', 'mesh-mount-id')
    mountId.textContent = root.dataset.mountId
    root.appendChild(mountId)

    const projectPath = document.createElement('div')
    projectPath.setAttribute('data-testid', 'mesh-project-path')
    projectPath.textContent = props.projectPath || ''
    root.appendChild(projectPath)

    const button = document.createElement('button')
    button.type = 'button'
    button.textContent = 'Focus Pane'
    button.setAttribute('data-testid', 'mesh-focus-trigger')
    button.onclick = () => props.onFocusPane?.('%2')
    root.appendChild(button)
  }),
}))

const ipc = await import('./lib/ipc.js')
const eventApi = await import('@tauri-apps/api/event')
const {
  loadDeferredProjectSelectionData,
  prefetchProjectSelectionData,
} = await import('./lib/projectSelection.js')
const { loadThemePreferences } = await import('./lib/shell/themePreferences.js')
const { setProjectContext } = await import('./lib/context/ProjectContext.js')

import Shell from './Shell.svelte'

describe('Shell mesh focus integration', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  beforeEach(() => {
    vi.clearAllMocks()
    meshTabMountCount = 0
    latestSidebarProps = {}
    mockSessionMap = new Map([
      ['/projects/mesh', [
        {
          tmux_session: 'taurhaus',
          tmux_window: '2',
          tmux_pane: '%2',
          project_path: '/projects/mesh',
        },
      ]],
    ])

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
        name: 'mesh',
        path: '/projects/mesh',
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
    ipc.getForegroundProject.mockResolvedValue(null)
    ipc.listClaudeSessions.mockResolvedValue([
      {
        tmuxSession: 'taurhaus',
        tmuxWindow: '2',
        tmuxPane: '%2',
        projectPath: '/projects/mesh',
      },
    ])

    loadThemePreferences.mockResolvedValue({
      codeThemeLight: 'github-light',
      codeThemeDark: 'github-dark-dimmed',
      darkMode: false,
    })

    loadDeferredProjectSelectionData.mockResolvedValue({
      detail: { ok: true, section: 'Project details', value: { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' } },
      commits: { ok: true, section: 'Recent commits', value: [] },
      latest: { ok: true, section: 'Latest session', value: null },
      sessionList: { ok: true, section: 'Session history', value: [] },
      readme: { ok: true, section: 'README', value: null },
      rels: { ok: true, section: 'Relationships', value: [] },
    })
    prefetchProjectSelectionData.mockResolvedValue(null)

    eventApi.listen.mockResolvedValue(() => {})
  })

  it('loads the initial project once on startup without a delayed duplicate selection batch', async () => {
    render(Shell)

    await waitFor(() => {
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(1)
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledWith('proj-1', expect.any(Object), expect.any(Object))
    })
  })

  it('resolves mesh pane focus to tmux coordinates, foreground project, and opens the terminal', async () => {
    render(Shell)

    await waitFor(() => {
      expect(ipc.listProjects).toHaveBeenCalled()
      expect(screen.getByTestId('tab-mesh')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('tab-mesh'))

    await waitFor(() => {
      expect(screen.getByTestId('mesh-focus-trigger')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('mesh-focus-trigger'))

    await waitFor(() => {
      expect(ipc.listClaudeSessions).toHaveBeenCalled()
      expect(latestSidebarProps.foregroundProjectId).toBe('proj-2')
      expect(ipc.navigateToSession).toHaveBeenCalledWith('taurhaus', '2', '%2', true)
    })
  })

  it('queries foreground project on mount and passes it to Sidebar', async () => {
    ipc.getForegroundProject.mockResolvedValueOnce('proj-2')

    render(Shell)

    await waitFor(() => {
      expect(ipc.getForegroundProject).toHaveBeenCalledTimes(1)
      expect(latestSidebarProps.foregroundProjectId).toBe('proj-2')
    })
  })

  it('updates foreground project immediately when the sidebar reports a session click', async () => {
    render(Shell)

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-foreground-trigger')).toBeInTheDocument()
      expect(latestSidebarProps.foregroundProjectId).toBeNull()
    })

    await fireEvent.click(screen.getByTestId('sidebar-foreground-trigger'))

    expect(latestSidebarProps.foregroundProjectId).toBe('proj-2')
  })

  it('prefetches a hovered project without changing the visible selection', async () => {
    render(Shell)

    await waitFor(() => {
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(1)
      expect(latestSidebarProps.actions?.onProjectHover).toEqual(expect.any(Function))
    })

    latestSidebarProps.actions.onProjectHover({
      id: 'proj-2',
      name: 'mesh',
      path: '/projects/mesh',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    expect(prefetchProjectSelectionData).toHaveBeenCalledWith(
      'proj-2',
      expect.any(Object),
      expect.any(Object)
    )
    const projectContext = setProjectContext.mock.calls.at(-1)?.[0]
    expect(projectContext.selectedProject?.id).toBe('proj-1')
  })

  it('updates foreground project from tmux focus events and clears it on detach', async () => {
    // Regression: commits a53ad31 and f9c1e89. The focus event used to carry raw
    // tmux coordinates from a hook-written file; the daemon hub now resolves the
    // project and the event carries project_id.
    ipc.isTauri.mockReturnValue(true)
    const handlers = new Map()
    eventApi.listen.mockImplementation((eventName, handler) => {
      handlers.set(eventName, handler)
      return Promise.resolve(() => {})
    })

    render(Shell)

    await waitFor(() => {
      expect(handlers.has('tmux-focus-changed')).toBe(true)
    })

    await handlers.get('tmux-focus-changed')({
      payload: { session: 'taurhaus', window: '2', pane_id: '%9', project_id: 'proj-2' },
    })

    await waitFor(() => {
      expect(latestSidebarProps.foregroundProjectId).toBe('proj-2')
    })

    await handlers.get('tmux-focus-changed')({ payload: null })

    await waitFor(() => {
      expect(latestSidebarProps.foregroundProjectId).toBeNull()
    })
  })

  it('keeps shell event listeners mounted once across startup and project selection churn', async () => {
    ipc.isTauri.mockReturnValue(true)
    eventApi.listen.mockResolvedValue(() => {})

    render(Shell)

    await waitFor(() => {
      expect(eventApi.listen).toHaveBeenCalledTimes(7)
    })

    const projectContext = setProjectContext.mock.calls.at(-1)?.[0]
    await projectContext.selectProject({
      id: 'proj-2',
      name: 'mesh',
      path: '/projects/mesh',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    await waitFor(() => {
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(2)
      expect(loadDeferredProjectSelectionData).toHaveBeenLastCalledWith(
        'proj-2',
        expect.any(Object),
        expect.any(Object)
      )
    })

    // Regression: after the Shell controller split, real-time updates stopped because
    // Tauri listeners were being managed from a reactive effect instead of a mount lifecycle.
    expect(eventApi.listen).toHaveBeenCalledTimes(7)
  })

  it('retries a deferred project load after daemon reconnects during a project switch', async () => {
    ipc.isTauri.mockReturnValue(true)
    ipc.getDaemonStatus.mockResolvedValue({ status: 'disconnected' })

    const handlers = new Map()
    eventApi.listen.mockImplementation((eventName, handler) => {
      handlers.set(eventName, handler)
      return Promise.resolve(() => {})
    })

    let proj2Attempts = 0
    loadDeferredProjectSelectionData.mockImplementation(async (projectId) => {
      if (projectId === 'proj-2') {
        proj2Attempts += 1
        if (proj2Attempts === 1) {
          return {
            detail: {
              ok: true,
              section: 'Project details',
              value: { id: 'proj-2', path: '/projects/mesh', name: 'mesh' },
            },
            commits: {
              ok: false,
              section: 'Recent commits',
              value: [],
              message: 'Daemon transport error: recent commits is unavailable for WSL UNC repositories without a connected daemon',
              retryableOnDaemonReconnect: true,
            },
            latest: { ok: true, section: 'Latest session', value: null },
            sessionList: { ok: true, section: 'Session history', value: [] },
            readme: { ok: true, section: 'README', value: null },
            rels: { ok: true, section: 'Relationships', value: [] },
          }
        }
        return {
          detail: {
            ok: true,
            section: 'Project details',
            value: { id: 'proj-2', path: '/projects/mesh', name: 'mesh' },
          },
          commits: { ok: true, section: 'Recent commits', value: [] },
          latest: { ok: true, section: 'Latest session', value: null },
          sessionList: { ok: true, section: 'Session history', value: [] },
          readme: { ok: true, section: 'README', value: null },
          rels: { ok: true, section: 'Relationships', value: [] },
        }
      }

      return {
        detail: {
          ok: true,
          section: 'Project details',
          value: { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' },
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
      expect(handlers.has('daemon-status')).toBe(true)
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(1)
      expect(screen.getByTestId('daemon-connecting-banner')).toBeInTheDocument()
    })

    const projectContext = setProjectContext.mock.calls.at(-1)?.[0]
    await projectContext.selectProject({
      id: 'proj-2',
      name: 'mesh',
      path: '/projects/mesh',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-2')
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(2)
    })

    expect(screen.queryByTestId('project-load-degraded-banner')).not.toBeInTheDocument()

    await handlers.get('daemon-status')({ payload: { status: 'connected' } })

    await waitFor(() => {
      expect(loadDeferredProjectSelectionData).toHaveBeenCalledTimes(3)
      expect(loadDeferredProjectSelectionData).toHaveBeenLastCalledWith(
        'proj-2',
        expect.any(Object),
        expect.any(Object)
      )
      expect(screen.queryByTestId('daemon-connecting-banner')).not.toBeInTheDocument()
    })

    expect(screen.queryByTestId('project-load-degraded-banner')).not.toBeInTheDocument()
  })

  it('remounts the mesh tab with the next project when switching projects', async () => {
    loadDeferredProjectSelectionData.mockImplementation(async (projectId) => ({
      detail: {
        ok: true,
        section: 'Project details',
        value: projectId === 'proj-2'
          ? { id: 'proj-2', path: '/projects/mesh', name: 'mesh' }
          : { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' },
      },
      commits: { ok: true, section: 'Recent commits', value: [] },
      latest: { ok: true, section: 'Latest session', value: null },
      sessionList: { ok: true, section: 'Session history', value: [] },
      readme: { ok: true, section: 'README', value: null },
      rels: { ok: true, section: 'Relationships', value: [] },
    }))

    render(Shell)

    await waitFor(() => {
      expect(screen.getByTestId('tab-mesh')).toBeInTheDocument()
    })

    const initialProjectContext = setProjectContext.mock.calls.at(-1)?.[0]
    await waitFor(() => {
      expect(initialProjectContext.selectedProject?.id).toBe('proj-1')
    })

    await fireEvent.click(screen.getByTestId('tab-mesh'))

    await waitFor(() => {
      expect(screen.getAllByTestId('mesh-focus-trigger').length).toBeGreaterThan(0)
      expect(lastTextByTestId('mesh-project-path')).toBe('/projects/taurhaus')
    })

    const projectContext = setProjectContext.mock.calls.at(-1)?.[0]
    await projectContext.selectProject({
      id: 'proj-2',
      name: 'mesh',
      path: '/projects/mesh',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-2')
      expect(screen.getByTestId('mock-overview')).toBeInTheDocument()
    })

    await fireEvent.click(screen.getByTestId('tab-mesh'))

    await waitFor(() => {
      expect(lastTextByTestId('mesh-project-path')).toBe('/projects/mesh')
    })

    await projectContext.selectProject({
      id: 'proj-1',
      name: 'taurhaus',
      path: '/projects/taurhaus',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-1')
    })

    const mountCountBeforeSwitch = meshTabMountCount

    await projectContext.selectProject({
      id: 'proj-2',
      name: 'mesh',
      path: '/projects/mesh',
      activityState: 'active',
      branch: 'main',
      isDirty: false,
    })

    await waitFor(() => {
      expect(projectContext.selectedProject?.id).toBe('proj-2')
    })

    expect(meshTabMountCount).toBeGreaterThan(mountCountBeforeSwitch)
    expect(lastTextByTestId('mesh-project-path')).toBe('/projects/mesh')
  })
})
