import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('./ipc.js', () => ({
  navigateToSession: vi.fn(),
  launchClaudeSession: vi.fn(),
  stopClaudeSession: vi.fn(),
  removeProject: vi.fn(),
  listClaudeAccounts: vi.fn(),
  setProjectClaudeAccount: vi.fn(),
  resolveClaudeLaunchAccount: vi.fn(),
  getSettings: vi.fn(),
}))

vi.mock('./sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

vi.mock('./sessionIndicator.js', () => ({
  hasLiveSession: vi.fn((session) => session?.state === 'active' || session?.state === 'idle'),
  rowTintForSessions: vi.fn(() => ''),
  toolIndicators: vi.fn(() => []),
}))

const {
  launchClaudeSession,
  listClaudeAccounts,
  setProjectClaudeAccount,
  stopClaudeSession,
  resolveClaudeLaunchAccount,
  getSettings,
} = await import('./ipc.js')
const { getSessionsForProject } = await import('./sessionStore.svelte.js')
const { claudeAccounts, resetClaudeAccountsForTest } = await import('./claudeAccounts.svelte.js')
import Sidebar from './Sidebar.svelte'

const PRIMARY = {
  id: 'account-1',
  email: 'stierms@gmail.com',
  display_name: 'Who',
  logged_in: true,
  is_default: true,
}
const SECOND = {
  id: 'account-2',
  email: 'm.stier@giesi.com',
  display_name: 'Matthias',
  logged_in: true,
  is_default: false,
}
const LOGGED_OUT = {
  id: 'account-3',
  email: 'work@example.com',
  display_name: 'Work',
  logged_in: false,
  is_default: false,
}

const PROJECT = {
  id: 'project-0',
  name: 'taurhaus',
  path: '/projects/taurhaus',
  activityState: 'active',
  branch: 'main',
}

function detected(accounts) {
  return { accounts, source: 'native', degraded: false, error: null }
}

/** Hover-open: a launch row keeps its click for the launch itself. */
async function hoverOpenSubmenu(testid) {
  await waitFor(() => expect(screen.getByTestId(testid)).toHaveAttribute('aria-haspopup', 'menu'))
  await fireEvent.mouseEnter(screen.getByTestId(testid))
  await waitFor(() => expect(screen.getByTestId('context-submenu')).toBeInTheDocument())
}

async function openProjectMenu(project = PROJECT) {
  render(Sidebar, { props: { projects: [project] } })
  await waitFor(() => expect(screen.getByTestId('project-item')).toBeInTheDocument())
  await fireEvent.contextMenu(screen.getByTestId('project-item'))
  // The menu builds from detected accounts, and detection is an IPC round trip.
  await waitFor(() => expect(listClaudeAccounts).toHaveBeenCalled())
}

describe('Sidebar account submenus', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetClaudeAccountsForTest()
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND, LOGGED_OUT]))
    setProjectClaudeAccount.mockResolvedValue(undefined)
    launchClaudeSession.mockResolvedValue({ ok: true })
    stopClaudeSession.mockResolvedValue(undefined)
    resolveClaudeLaunchAccount.mockResolvedValue({ needsChoice: true })
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: null } })
    getSessionsForProject.mockImplementation(() => [])
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('gives every Claude launch item a submenu, and no other tool one', async () => {
    await openProjectMenu()

    for (const testid of [
      'menu-item-new-claude-session',
      'menu-item-continue-claude',
      'menu-item-resume-claude',
    ]) {
      await waitFor(() =>
        expect(screen.getByTestId(testid)).toHaveAttribute('aria-haspopup', 'menu')
      )
    }

    expect(screen.getByTestId('menu-item-new-codex-session')).not.toHaveAttribute('aria-haspopup')
    expect(screen.getByTestId('menu-item-resume-gemini')).not.toHaveAttribute('aria-haspopup')
  })

  it('offers no submenu when the host has only one usable account', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, LOGGED_OUT]))

    await openProjectMenu()

    await waitFor(() => expect(claudeAccounts.accounts).toHaveLength(2))
    expect(screen.getByTestId('menu-item-new-claude-session')).not.toHaveAttribute('aria-haspopup')
    expect(screen.queryByTestId('menu-item-claude-account')).not.toBeInTheDocument()
  })

  // Regression: 6ec843e gave a subscription signed into two config dirs a row
  // per dir, labelled by the dir. Both rows carry the same account uuid — the
  // only address a launch has — so the second row launched the first dir while
  // the menu ticked both. One subscription is one choice, and one choice is no
  // question to ask.
  it('offers no submenu when one subscription is signed into two config dirs', async () => {
    listClaudeAccounts.mockResolvedValue(
      detected([
        { ...PRIMARY, config_dir: '/home/user/.claude' },
        { ...PRIMARY, is_default: false, config_dir: '/home/user/.claude-copy' },
      ])
    )

    await openProjectMenu()

    await waitFor(() => expect(claudeAccounts.accounts).toHaveLength(1))
    expect(screen.getByTestId('menu-item-new-claude-session')).not.toHaveAttribute('aria-haspopup')
    expect(screen.queryByTestId('menu-item-claude-account')).not.toBeInTheDocument()
  })

  // Regression: 74c7761 pinned the project to the account a launch row named.
  // The launch rows are per-launch overrides; the `Claude account` submenu is
  // where a project chooses the subscription it keeps.
  it('launches immediately on the account a child names, and pins nothing', async () => {
    await openProjectMenu()

    await hoverOpenSubmenu('menu-item-new-claude-session')
    await fireEvent.mouseDown(screen.getByTestId('submenu-item-matthias'))

    await waitFor(() => {
      expect(launchClaudeSession).toHaveBeenCalledWith('project-0', 'fresh', 'claude', 'account-2')
    })
    expect(setProjectClaudeAccount).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toBe(null)
  })

  // Regression: 74c7761 built a row's label from the display name alone, and
  // the flyout keyed its rows by that label. Two subscriptions of the same
  // named user crashed the submenu instead of rendering it.
  it('renders both rows when two accounts share a display name', async () => {
    listClaudeAccounts.mockResolvedValue(
      detected([PRIMARY, { ...SECOND, display_name: 'Who' }, LOGGED_OUT])
    )

    await openProjectMenu()
    await hoverOpenSubmenu('menu-item-new-claude-session')

    const rows = within(screen.getByTestId('context-submenu')).getAllByRole('menuitemradio')
    expect(rows.map((row) => row.textContent.trim())).toEqual(
      expect.arrayContaining([
        expect.stringContaining('Who (stierms@gmail.com)'),
        expect.stringContaining('Who (m.stier@giesi.com)'),
      ])
    )
    expect(new Set(rows.map((row) => row.dataset.testid)).size).toBe(rows.length)
  })

  // Regression: 74c7761 ticked the same account on every launch row. Continue
  // and Resume follow the transcript's config dir, which the backend resolves,
  // so a tick there claims an account the click may not use.
  it('ticks the account a fresh launch uses, and none on the history rows', async () => {
    await openProjectMenu()

    await hoverOpenSubmenu('menu-item-new-claude-session')
    expect(screen.getByTestId('submenu-check-who')).toHaveAttribute('data-checked', 'true')

    await fireEvent.mouseEnter(screen.getByTestId('menu-item-resume-claude'))
    await waitFor(() =>
      expect(screen.getByTestId('menu-item-resume-claude')).toHaveAttribute(
        'aria-expanded',
        'true'
      )
    )
    const checks = within(screen.getByTestId('context-submenu')).getAllByRole('menuitemradio')
    for (const row of checks) {
      expect(row).toHaveAttribute('aria-checked', 'false')
    }
  })

  it('marks the account the launch would use today and meters each one', async () => {
    listClaudeAccounts.mockResolvedValue(
      detected([
        {
          ...PRIMARY,
          usage: {
            five_hour: { used_percentage: 3, resets_at: Math.floor(Date.now() / 1000) + 3600 },
            seven_day: { used_percentage: 27, resets_at: Math.floor(Date.now() / 1000) + 90_000 },
            observed_at: new Date().toISOString(),
          },
        },
        SECOND,
        LOGGED_OUT,
      ])
    )

    await openProjectMenu()

    await hoverOpenSubmenu('menu-item-new-claude-session')

    expect(screen.getByTestId('submenu-check-who')).toHaveAttribute('data-checked', 'true')
    expect(screen.getByTestId('submenu-item-who')).toHaveTextContent('5h 3% · 7d 27%')
    expect(screen.getByTestId('submenu-item-work')).toBeDisabled()
    expect(screen.getByTestId('submenu-item-work')).toHaveTextContent('not logged in')
  })

  it('clicking the parent row still asks when nothing has been chosen', async () => {
    await openProjectMenu()

    await waitFor(() =>
      expect(screen.getByTestId('menu-item-new-claude-session')).toHaveAttribute('aria-haspopup')
    )
    await fireEvent.mouseDown(screen.getByTestId('menu-item-new-claude-session'))

    await waitFor(() => {
      expect(claudeAccounts.pending).toMatchObject({ projectId: 'project-0', mode: 'fresh' })
    })
    expect(launchClaudeSession).not.toHaveBeenCalled()
  })

  it('pins from the Claude account submenu without launching, and clears it again', async () => {
    await openProjectMenu()

    await hoverOpenSubmenu('menu-item-claude-account')
    await fireEvent.mouseDown(screen.getByTestId('submenu-item-matthias'))

    await waitFor(() => {
      expect(setProjectClaudeAccount).toHaveBeenCalledWith('project-0', 'account-2')
    })
    expect(launchClaudeSession).not.toHaveBeenCalled()

    await fireEvent.contextMenu(screen.getByTestId('project-item'))
    await hoverOpenSubmenu('menu-item-claude-account')
    await fireEvent.mouseDown(screen.getByTestId('submenu-item-use-default'))

    await waitFor(() => {
      expect(setProjectClaudeAccount).toHaveBeenLastCalledWith('project-0', null)
    })
  })

  it('restarts a live session on the account the submenu names', async () => {
    getSessionsForProject.mockImplementation(() => [
      { state: 'active', cli_tool: 'claude', tmux_pane: '%9', tmux_session: 'team', tmux_window: '2' },
    ])

    await openProjectMenu()

    await hoverOpenSubmenu('menu-item-restart-claude')
    await fireEvent.mouseDown(screen.getByTestId('submenu-item-matthias'))

    await waitFor(() => {
      expect(stopClaudeSession).toHaveBeenCalledWith('%9', 'claude')
      expect(launchClaudeSession).toHaveBeenCalledWith('project-0', 'fresh', 'claude', 'account-2')
    })
  })

  it('is reachable from the keyboard: ArrowRight opens the submenu, Enter launches', async () => {
    await openProjectMenu()

    await waitFor(() =>
      expect(screen.getByTestId('menu-item-new-claude-session')).toHaveAttribute('aria-haspopup')
    )

    await fireEvent.mouseEnter(screen.getByTestId('menu-item-new-claude-session'))
    await fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.getByTestId('context-submenu')).toBeInTheDocument()

    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'ArrowDown' })
    await fireEvent.keyDown(window, { key: 'Enter' })

    await waitFor(() => {
      expect(launchClaudeSession).toHaveBeenCalledWith('project-0', 'fresh', 'claude', 'account-2')
    })
  })
})
