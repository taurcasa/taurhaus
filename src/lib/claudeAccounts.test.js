import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ipc.js', () => ({
  listClaudeAccounts: vi.fn(),
  setProjectClaudeAccount: vi.fn(),
  launchClaudeSession: vi.fn(),
  getSettings: vi.fn(),
}))

const { listClaudeAccounts, setProjectClaudeAccount, launchClaudeSession, getSettings } =
  await import('./ipc.js')
const {
  claudeAccounts,
  effectiveClaudeAccountId,
  loggedInAccounts,
  refreshClaudeAccounts,
  requestClaudeLaunch,
  resolveChooserAccounts,
  resetClaudeAccountsForTest,
  setGlobalClaudeAccount,
} = await import('./claudeAccounts.svelte.js')

const PRIMARY = {
  id: 'account-1',
  email: 'stierms@gmail.com',
  display_name: 'Who',
  organization: "stierms@gmail.com's Organization",
  seat_tier: 'claude_max',
  logged_in: true,
  is_default: true,
  config_dir: '/home/user/.claude',
}

const SECOND = {
  id: 'account-2',
  email: 'm.stier@giesi.com',
  display_name: 'Matthias',
  organization: "m.stier@giesi.com's Organization",
  seat_tier: 'claude_max',
  logged_in: true,
  is_default: false,
  config_dir: '/home/user/.claude-account2',
}

const THIRD = {
  id: 'account-3',
  email: 'third@example.com',
  display_name: 'Third',
  organization: null,
  seat_tier: null,
  logged_in: true,
  is_default: false,
  config_dir: '/home/user/.claude-third',
}

describe('claudeAccounts store', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetClaudeAccountsForTest()
    setProjectClaudeAccount.mockResolvedValue(undefined)
    launchClaudeSession.mockResolvedValue({ tmux_pane: '%1' })
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: null } })
  })

  it('keeps a logged-out account visible for the chooser but out of the count', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, { ...SECOND, logged_in: false }])

    await refreshClaudeAccounts()

    expect(claudeAccounts.accounts).toHaveLength(2)
    expect(resolveChooserAccounts().map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(loggedInAccounts()).toHaveLength(1)
  })

  it('launches straight away when only one account is logged in', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY])
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('never asks for a non-Claude tool', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'codex' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'codex', null)
  })

  it('asks once when two accounts are logged in and the project stored no choice', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1', mode: 'fresh' })
  })

  it('remembers the choice for the project and launches on it', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', true)

    expect(setProjectClaudeAccount).toHaveBeenCalledWith('p1', 'account-2')
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    expect(claudeAccounts.pending).toBe(null)
  })

  it('does not store the choice when the user unticks remember', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', false)

    expect(setProjectClaudeAccount).not.toHaveBeenCalled()
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
  })

  it('skips the chooser once the project has a stored account', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()

    await requestClaudeLaunch({
      project: { id: 'p1', claude_account_id: 'account-2' },
      mode: 'continue',
      tool: 'claude',
    })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'continue', 'claude', null)
  })

  it('cancelling the chooser launches nothing', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    claudeAccounts.pending.cancel()

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toBe(null)
  })

  it('a detection failure never blocks the launch', async () => {
    listClaudeAccounts.mockRejectedValue(new Error('daemon down'))
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  // Regression: c982822 decided whether to ask from the project object the
  // caller happened to hold and updated nothing after storing the answer, so
  // the same project asked again on every launch of the app session.
  it('asks once and never again once the choice is remembered', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    const project = { id: 'p1' }
    await requestClaudeLaunch({ project, mode: 'fresh', tool: 'claude' })
    await claudeAccounts.pending.confirm('account-2', true)

    await requestClaudeLaunch({ project, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledTimes(2)
    expect(effectiveClaudeAccountId(project)).toBe('account-2')
  })

  it('a failed store drops the remembered choice again', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    setProjectClaudeAccount.mockRejectedValue(new Error('db locked'))
    const project = { id: 'p1' }
    await requestClaudeLaunch({ project, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', true)

    expect(effectiveClaudeAccountId(project)).toBe(null)
  })

  // Regression: c982822 left the configured global default to the backend
  // alone. The chooser asked anyway, and Enter pinned the project to whichever
  // account happened to sit in the default config dir.
  it('never asks when a global default account is configured', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: 'account-2' } })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(claudeAccounts.defaultAccountId).toBe('account-2')
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('asks again when the configured global default is no longer usable', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, { ...SECOND, logged_in: false }, THIRD])
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: 'account-2' } })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  it('a default chosen in settings takes effect without a reload', async () => {
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])
    await refreshClaudeAccounts()

    setGlobalClaudeAccount('account-2')

    expect(claudeAccounts.defaultAccountId).toBe('account-2')
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })
    expect(claudeAccounts.pending).toBe(null)
  })

  // Regression: c982822 read the account list synchronously while detection
  // was still in flight, so a launch clicked during startup silently ran on
  // the backend default instead of asking.
  it('waits for detection that is still in flight', async () => {
    let publish
    listClaudeAccounts.mockReturnValue(
      new Promise((resolve) => {
        publish = resolve
      })
    )
    const detecting = refreshClaudeAccounts()

    const launching = requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })
    publish([PRIMARY, SECOND])
    await detecting
    await launching

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: c982822 cached a failed detection as an empty account list for
  // the full 60 s TTL, so a daemon that connected a moment later could not
  // restore the chooser.
  it('detects again after a failure instead of caching the empty list', async () => {
    listClaudeAccounts.mockRejectedValueOnce(new Error('daemon down'))
    await refreshClaudeAccounts()
    listClaudeAccounts.mockResolvedValue([PRIMARY, SECOND])

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })
})
