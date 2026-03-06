import { beforeEach, describe, expect, it, vi } from 'vitest'
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

vi.mock('./lib/AddProjectModal.svelte', () => ({
  default: createMockComponent('add-project', () => {}),
}))

vi.mock('./lib/FirstRunWizard.svelte', () => ({
  default: createMockComponent('first-run', () => {}),
}))

vi.mock('./lib/components/MeshTab.svelte', () => ({
  default: createMockComponent('mesh-tab', (root, props) => {
    const button = document.createElement('button')
    button.type = 'button'
    button.textContent = 'Focus Pane'
    button.setAttribute('data-testid', 'mesh-focus-trigger')
    button.onclick = () => props.onFocusPane?.('%2')
    root.appendChild(button)
  }),
}))

const ipc = await import('./lib/ipc.js')
const { loadProjectSelectionData } = await import('./lib/projectSelection.js')
const { loadThemePreferences } = await import('./lib/shell/themePreferences.js')

import Shell from './Shell.svelte'

describe('Shell mesh focus integration', () => {
  beforeEach(() => {
    vi.clearAllMocks()

    ipc.listProjects.mockResolvedValue([
      {
        id: 'proj-1',
        name: 'taurhaus',
        path: '/projects/taurhaus',
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
    ipc.listClaudeSessions.mockResolvedValue([
      {
        tmuxSession: 'taurhaus',
        tmuxWindow: '2',
        tmuxPane: '%2',
      },
    ])

    loadThemePreferences.mockResolvedValue({
      codeThemeLight: 'github-light',
      codeThemeDark: 'github-dark-dimmed',
      darkMode: false,
    })

    loadProjectSelectionData.mockResolvedValue({
      detail: { ok: true, section: 'Project details', value: { id: 'proj-1', path: '/projects/taurhaus', name: 'taurhaus' } },
      commits: { ok: true, section: 'Recent commits', value: [] },
      latest: { ok: true, section: 'Latest session', value: null },
      sessionList: { ok: true, section: 'Session history', value: [] },
      readme: { ok: true, section: 'README', value: null },
      rels: { ok: true, section: 'Relationships', value: [] },
    })
  })

  it('resolves mesh pane focus to tmux coordinates and opens the terminal', async () => {
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
      expect(ipc.navigateToSession).toHaveBeenCalledWith('taurhaus', '2', '%2', true)
    })
  })
})
