import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render } from '@testing-library/svelte'
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

vi.mock('./lib/Sidebar.svelte', () => ({ default: createMockComponent('sidebar') }))
vi.mock('./lib/OverviewTab.svelte', () => ({ default: createMockComponent('overview') }))
vi.mock('./lib/FilesTab.svelte', () => ({ default: createMockComponent('files') }))
vi.mock('./lib/TaskBoard.svelte', () => ({ default: createMockComponent('tasks') }))
vi.mock('./lib/GitTab.svelte', () => ({ default: createMockComponent('git') }))
vi.mock('./lib/SearchOverlay.svelte', () => ({ default: createMockComponent('search') }))
vi.mock('./lib/Settings.svelte', () => ({ default: createMockComponent('settings') }))
vi.mock('./lib/AddProjectModal.svelte', () => ({ default: createMockComponent('add-project') }))
vi.mock('./lib/FirstRunWizard.svelte', () => ({ default: createMockComponent('first-run') }))
vi.mock('./lib/components/MeshTab.svelte', () => ({ default: createMockComponent('mesh-tab') }))

const { refreshAccountRelationships, refreshAccounts, refreshResolvedBases, refreshUsage } =
  await import('./lib/accounts.svelte.js')

import Shell from './Shell.svelte'

const toolsIn = (calls) => new Set(calls.map(([tool]) => tool))
const toolsAsked = (mock) => toolsIn(mock.mock.calls)

describe('Shell ambient account synchronisation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  // Regression: 971d9643 keyed the shell's only account refresh to daemon-status
  // changes over account-selectable tools alone and never refreshed usage, so
  // the footer magnitude sat at its startup value and Antigravity was absent
  // until someone hovered the accounts button.
  it('reads every registry tool, usage included, without a hover', async () => {
    render(Shell)
    await vi.advanceTimersByTimeAsync(10)

    expect(toolsAsked(refreshAccounts)).toEqual(new Set(['claude', 'codex', 'agy', 'grok']))
    expect(toolsAsked(refreshUsage)).toEqual(new Set(['claude', 'codex', 'agy']))
  })

  it('re-reads what the backend poller has observed since, without forcing it', async () => {
    render(Shell)
    await vi.advanceTimersByTimeAsync(10)
    const opening = refreshAccounts.mock.calls.length
    const openingProviderReads = refreshUsage.mock.calls.length

    await vi.advanceTimersByTimeAsync(60_000)

    const tick = refreshAccounts.mock.calls.slice(opening)
    expect(toolsIn(tick)).toEqual(new Set(['claude', 'codex', 'agy', 'grok']))
    expect(tick.every(([, options]) => options?.force === true)).toBe(true)
    // The poller decides how often a subscription is worth asking again; the
    // chrome only reads what it has already observed.
    expect(refreshUsage.mock.calls.length).toBe(openingProviderReads)
  })

  // Regression: 6556676e brought the ambient chrome level with detection but
  // never with what the launch commands select, so the footer judged the
  // default directory relevant while an alias sent every launch elsewhere.
  it('reads what the launch commands select for the tools that can switch', async () => {
    render(Shell)
    await vi.advanceTimersByTimeAsync(10)

    expect(toolsAsked(refreshResolvedBases)).toEqual(new Set(['claude', 'codex', 'grok']))
  })

  // Regression: 6556676e refreshed the relationship index only on the opening
  // pass, so a pin made from Overview or the sidebar left the footer's calm or
  // warning state as it was until somebody opened Accounts.
  it('keeps the relationship index level on the recurring pass', async () => {
    render(Shell)
    await vi.advanceTimersByTimeAsync(10)
    const opening = refreshAccountRelationships.mock.calls.length
    expect(opening).toBeGreaterThan(0)

    await vi.advanceTimersByTimeAsync(60_000)

    const tick = refreshAccountRelationships.mock.calls.slice(opening)
    expect(toolsIn(tick)).toEqual(new Set(['claude', 'codex', 'agy', 'grok']))
  })
})
