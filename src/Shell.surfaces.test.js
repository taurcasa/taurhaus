import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

function createMockComponent(name) {
  return function MockComponent(target) {
    const root = document.createElement('div')
    root.setAttribute('data-testid', `mock-${name}`)
    if (target.nodeType === Node.ELEMENT_NODE) {
      target.appendChild(root)
    } else {
      target.parentNode.insertBefore(root, target)
    }
    return {
      $set() {},
      $destroy() {
        root.remove()
      },
    }
  }
}

vi.mock('./lib/ipc.js', async (importOriginal) => {
  const actual = await importOriginal()
  return {
    ...actual,
    isTauri: vi.fn(() => false),
    isFirstRun: vi.fn(() => Promise.resolve(false)),
    listProjects: vi.fn(() => Promise.resolve([])),
    getSettings: vi.fn(() => Promise.resolve({ dark_mode: false, code_theme: null })),
    getDaemonStatus: vi.fn(() => Promise.resolve('connected')),
    checkDaemonInstallStatus: vi.fn(() =>
      Promise.resolve({ installed: true, needs_update: false })
    ),
    getPlatform: vi.fn(() => Promise.resolve('linux')),
    getForegroundProject: vi.fn(() => Promise.resolve(null)),
    listClaudeSessions: vi.fn(() => Promise.resolve([])),
  }
})

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}))

vi.mock('./lib/sessionStore.svelte.js', () => ({
  DEFAULT_TAURI_POLL_INTERVAL_MS: 5000,
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
  getSessions: vi.fn(() => new Map()),
  applyDaemonSessionUpdate: vi.fn(),
  hydrateFromBackend: vi.fn(),
  markSessionPresenceStale: vi.fn(),
  startPolling: vi.fn(),
  stopPolling: vi.fn(),
}))

vi.mock('./lib/shell/themePreferences.js', () => ({
  loadThemePreferences: vi.fn(() =>
    Promise.resolve({ codeThemeLight: 'github-light', codeThemeDark: 'github-dark-dimmed', darkMode: false })
  ),
  persistDarkModePreference: vi.fn(),
}))

vi.mock('./lib/accounts.svelte.js', () => ({
  accountState: vi.fn(() => ({
    accounts: [],
    resolvedBases: [],
    resolvingBases: false,
    degraded: false,
    defaultAccountId: null,
    projectChoices: {},
    relationships: {},
    generation: 0,
    pending: null,
  })),
  activeAccountId: vi.fn(() => null),
  effectiveAccount: vi.fn(() => ({ account: null, origin: 'default_config_dir' })),
  forgetResolvedBases: vi.fn(),
  launchAccountNotice: vi.fn(() => null),
  launchFollowsHistory: vi.fn(() => false),
  loggedInAccounts: vi.fn(() => []),
  opaqueBaseNotice: vi.fn(() => ''),
  pendingAccountChoice: vi.fn(() => null),
  previewAccount: vi.fn(() => Promise.resolve(null)),
  refreshAccountRelationships: vi.fn(() => Promise.resolve()),
  refreshAccounts: vi.fn(() => Promise.resolve()),
  refreshResolvedBases: vi.fn(() => Promise.resolve()),
  refreshUsage: vi.fn(() => Promise.resolve()),
  rememberChoice: vi.fn(() => Promise.resolve()),
  requestLaunch: vi.fn(() => Promise.resolve()),
  resetAccountsForTest: vi.fn(),
  resolveChooserAccounts: vi.fn(() => []),
  setDefaultAccount: vi.fn(),
  setGlobalDefault: vi.fn(() => Promise.resolve()),
}))

// The sidebar stays real: these tests exercise the footer keys and project
// rows against the Shell's surface state machine. Everything behind the main
// panel is mocked away.
vi.mock('./lib/OverviewTab.svelte', () => ({ default: createMockComponent('overview') }))
vi.mock('./lib/FilesTab.svelte', () => ({ default: createMockComponent('files') }))
vi.mock('./lib/TaskBoard.svelte', () => ({ default: createMockComponent('tasks') }))
vi.mock('./lib/GitTab.svelte', () => ({ default: createMockComponent('git') }))
vi.mock('./lib/SearchOverlay.svelte', () => ({ default: createMockComponent('search') }))
vi.mock('./lib/Settings.svelte', () => ({ default: createMockComponent('settings') }))
vi.mock('./lib/ProjectsTakeover.svelte', () => ({ default: createMockComponent('projects-takeover') }))
vi.mock('./lib/FirstRunWizard.svelte', () => ({ default: createMockComponent('first-run') }))
vi.mock('./lib/components/MeshTab.svelte', () => ({ default: createMockComponent('mesh-tab') }))

const { listProjects } = await import('./lib/ipc.js')

import Shell from './Shell.svelte'

const project = {
  id: 'p1',
  name: 'Fixture Project',
  path: '/projects/fixture',
  activityState: 'active',
  branch: 'main',
  isDirty: false,
}

// The Shell-level oracle is the real titlebar: an open utility surface
// replaces the project tabs with its takeover label. (The mocked surface
// components cannot signal teardown — Svelte 5 never calls a legacy mock's
// $destroy — so presence of tabs is the reliable closed-state signal.)
function surfaceLabelVisible(label) {
  return screen.queryByText(label) !== null && screen.queryAllByRole('tab').length === 0
}

async function openProjectsTakeover() {
  await waitFor(() => {
    expect(screen.getByTestId('manage-projects-btn')).toBeInTheDocument()
  })
  await fireEvent.click(screen.getByTestId('manage-projects-btn'))
  await waitFor(() => {
    expect(surfaceLabelVisible('Projects')).toBe(true)
  })
}

describe('Shell utility-surface state machine', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    listProjects.mockResolvedValue([project])
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('toggles the Projects takeover from the footer key', async () => {
    render(Shell)

    await openProjectsTakeover()

    // Second click on the key closes the surface — the key is a toggle like
    // its two siblings, so the pulled state never becomes a dead control.
    await fireEvent.click(screen.getByTestId('manage-projects-btn'))
    await waitFor(() => {
      expect(screen.getAllByRole('tab').length).toBeGreaterThan(0)
    })
  })

  it('keeps open-only semantics on the sidebar empty-state scan action', async () => {
    listProjects.mockResolvedValue([])
    render(Shell)

    await waitFor(() => {
      expect(screen.getByTestId('sidebar-empty-scan')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('sidebar-empty-scan'))
    await waitFor(() => {
      expect(surfaceLabelVisible('Projects')).toBe(true)
    })

    // A second invocation of the open-only entry point leaves the surface
    // open rather than toggling it away.
    await fireEvent.click(screen.getByTestId('sidebar-empty-scan'))
    expect(surfaceLabelVisible('Projects')).toBe(true)
  })

  it('closes any open utility surface when a project is selected', async () => {
    render(Shell)

    await openProjectsTakeover()

    // Clicking a project row must always show that project: the takeover
    // closes instead of leaving the selection change invisible.
    await fireEvent.click(screen.getByTestId('project-item'))
    await waitFor(() => {
      expect(screen.getAllByRole('tab').length).toBeGreaterThan(0)
    })

    // The row is pulled again — the panel is showing this project.
    await waitFor(() => {
      expect(screen.getByTestId('project-item').className).toContain('sidebar-row-pulled')
    })
  })

  it('closes Settings when a project is selected', async () => {
    render(Shell)

    await waitFor(() => {
      expect(screen.getByTestId('settings-toggle')).toBeInTheDocument()
    })
    await fireEvent.click(screen.getByTestId('settings-toggle'))
    await waitFor(() => {
      expect(surfaceLabelVisible('Settings')).toBe(true)
    })

    await fireEvent.click(screen.getByTestId('project-item'))
    await waitFor(() => {
      expect(screen.getAllByRole('tab').length).toBeGreaterThan(0)
    })
  })
})
