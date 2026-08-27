import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ipc.js', () => ({
  listClaudeAccounts: vi.fn(),
  setProjectClaudeAccount: vi.fn(),
  launchClaudeSession: vi.fn(),
  resolveClaudeLaunchAccount: vi.fn(),
  getSettings: vi.fn(),
}))

const {
  listClaudeAccounts,
  setProjectClaudeAccount,
  launchClaudeSession,
  resolveClaudeLaunchAccount,
  getSettings,
} = await import('./ipc.js')
const {
  activeClaudeAccountId,
  claudeAccounts,
  effectiveClaudeAccountId,
  loggedInAccounts,
  refreshClaudeAccounts,
  refreshClaudeAccountUsage,
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

/** What the backend answers when detection ran. */
const detected = (accounts) => ({
  accounts,
  source: 'native',
  degraded: false,
  error: null,
})

/** What it answers when the daemon could not be asked at all. */
const degraded = (error = 'The WSL daemon is not reachable') => ({
  accounts: [],
  source: 'daemon',
  degraded: true,
  error,
})

describe('claudeAccounts store', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetClaudeAccountsForTest()
    setProjectClaudeAccount.mockResolvedValue(undefined)
    launchClaudeSession.mockResolvedValue({ tmux_pane: '%1' })
    resolveClaudeLaunchAccount.mockResolvedValue({
      accountId: null,
      source: 'default_config_dir',
      needsChoice: true,
    })
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: null } })
  })

  it('keeps a logged-out account visible for the chooser but out of the count', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }]))

    await refreshClaudeAccounts()

    expect(claudeAccounts.accounts).toHaveLength(2)
    expect(resolveChooserAccounts().map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(loggedInAccounts()).toHaveLength(1)
  })

  it('launches straight away when only one account is logged in', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY]))
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('never asks for a non-Claude tool', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'codex' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'codex', null)
  })

  it('asks once when two accounts are logged in and the project stored no choice', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1', mode: 'fresh' })
  })

  it('remembers the choice for the project and launches on it', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', true)

    expect(setProjectClaudeAccount).toHaveBeenCalledWith('p1', 'account-2')
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    expect(claudeAccounts.pending).toBe(null)
  })

  it('does not store the choice when the user unticks remember', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()
    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', false)

    expect(setProjectClaudeAccount).not.toHaveBeenCalled()
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
  })

  it('skips the chooser once the project has a stored account', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
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
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
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
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    const project = { id: 'p1' }
    await requestClaudeLaunch({ project, mode: 'fresh', tool: 'claude' })
    await claudeAccounts.pending.confirm('account-2', true)

    await requestClaudeLaunch({ project, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledTimes(2)
    expect(effectiveClaudeAccountId(project)).toBe('account-2')
  })

  it('a failed store drops the remembered choice again', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
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
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: 'account-2' } })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(claudeAccounts.defaultAccountId).toBe('account-2')
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('asks again when the configured global default is no longer usable', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }, THIRD]))
    getSettings.mockResolvedValue({ terminal: { claude_default_account_id: 'account-2' } })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: 79be608 folded usage into account detection, which the store
  // caches for a minute and OverviewTab asked for once on mount. Every later
  // status-line report was invisible until something forced detection again,
  // so the chip a user opened to compare subscriptions showed mount-time
  // numbers — for a feature whose whole point is the current one.
  it('refreshes usage without waiting for the detection cache to expire', async () => {
    listClaudeAccounts.mockResolvedValue(
      detected([PRIMARY, { ...SECOND, usage: null }])
    )
    await refreshClaudeAccounts()
    expect(claudeAccounts.accounts[1].usage).toBe(null)

    const usage = {
      five_hour: { used_percentage: 81, resets_at: 1787784600 },
      seven_day: { used_percentage: 44, resets_at: 1788300000 },
      observed_at: '2026-08-27T12:00:00Z',
    }
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage }]))

    await refreshClaudeAccountUsage()

    expect(claudeAccounts.accounts[1].usage).toEqual(usage)
    // The accounts themselves are detection's answer, not this call's.
    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
  })

  // Regression: a574720 copied every account's usage from the report, `null`
  // included. The sink cannot be read while a compacting writer holds it, and
  // the backend answers such a refresh with no numbers at all — so the meter on
  // screen went blank for an account whose record was still on disk. The
  // numbers carry the moment they were observed, so keeping the last ones is
  // the honest answer: the meter labels its own age.
  it('keeps the numbers it has when a refresh brings none', async () => {
    const usage = {
      five_hour: { used_percentage: 81, resets_at: 1787784600 },
      seven_day: { used_percentage: 44, resets_at: 1788300000 },
      observed_at: '2026-08-27T12:00:00Z',
    }
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage }]))
    await refreshClaudeAccounts()
    expect(claudeAccounts.accounts[1].usage).toEqual(usage)

    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage: null }]))
    await refreshClaudeAccountUsage()

    expect(claudeAccounts.accounts[1].usage).toEqual(usage)

    // Detection answers with the same numbers from the same file, so it keeps
    // them for the same reason.
    await refreshClaudeAccounts({ force: true })
    expect(claudeAccounts.accounts[1].usage).toEqual(usage)
  })

  it('keeps the accounts it knows when a usage refresh cannot run', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()

    listClaudeAccounts.mockResolvedValue(degraded())
    await refreshClaudeAccountUsage()

    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
  })

  it('reads usage again before asking which subscription to launch on', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()
    listClaudeAccounts.mockClear()

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
    expect(listClaudeAccounts).toHaveBeenCalled()
  })

  it('a default chosen in settings takes effect without a reload', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
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
    publish(detected([PRIMARY, SECOND]))
    await detecting
    await launching

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: c982822 applied the chooser to every mode. `--resume` only
  // sees the history of the config dir it runs in, and the backend derives
  // that dir from the project's last transcript — but an explicit answer from
  // the chooser outranks it, so a resume was pinned to whichever account the
  // user picked in a dialog that should never have opened.
  it('never asks for a resume the backend can already place', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    resolveClaudeLaunchAccount.mockResolvedValue({
      accountId: 'account-2',
      source: 'session',
      needsChoice: false,
    })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'resume', 'claude', null)
  })

  it('asks for a resume the backend cannot place', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    resolveClaudeLaunchAccount.mockResolvedValue({
      accountId: null,
      source: 'default_config_dir',
      needsChoice: true,
    })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1', mode: 'resume' })
  })

  // Regression: c982822 treated any stored project account id as an answer.
  // A pinned account that logged out is not one: the backend refuses it, and
  // with the default config dir signed out too the launch landed on an account
  // nobody chose while several usable ones waited.
  it('asks again when the stored project account can no longer run', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }, THIRD]))

    await requestClaudeLaunch({
      project: { id: 'p1', claude_account_id: 'account-2' },
      mode: 'fresh',
      tool: 'claude',
    })

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: 518aace let the backend answer every daemon failure with a
  // successful empty list. The store believed it: the chooser stopped asking,
  // the chip disappeared, and launches ran on whichever subscription the
  // default config dir happened to hold — while both accounts were still
  // signed in and nothing on screen said otherwise.
  it('keeps the accounts it last knew when detection is degraded', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()

    listClaudeAccounts.mockResolvedValue(degraded())
    await refreshClaudeAccounts({ force: true })

    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(claudeAccounts.degraded).toBe(true)
  })

  it('still asks which subscription to use while detection is degraded', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()
    listClaudeAccounts.mockResolvedValue(degraded())
    await refreshClaudeAccounts({ force: true })

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(launchClaudeSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  it('detects again after a degraded answer instead of caching it', async () => {
    listClaudeAccounts.mockResolvedValueOnce(degraded())
    await refreshClaudeAccounts()
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

    await refreshClaudeAccounts()

    expect(claudeAccounts.degraded).toBe(false)
    expect(claudeAccounts.accounts).toHaveLength(2)
  })

  // Regression: 74c7761 asked for detection on every context-menu opening, and
  // a degraded answer warned every time. Every warn crosses the logging bridge,
  // so right-clicking during a daemon outage wrote an unbounded stream of the
  // same line. The outage is one event; the recovery is the next one.
  it('warns once while detection stays degraded, and again when it returns', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    listClaudeAccounts.mockResolvedValue(degraded())

    await refreshClaudeAccounts()
    await refreshClaudeAccounts({ force: true })
    await refreshClaudeAccounts({ force: true })

    const degradedWarnings = () =>
      warn.mock.calls.filter(([message]) =>
        String(message).includes('Claude account detection is unavailable')
      ).length
    expect(degradedWarnings()).toBe(1)

    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts({ force: true })
    listClaudeAccounts.mockResolvedValue(degraded())
    await refreshClaudeAccounts({ force: true })

    expect(degradedWarnings()).toBe(2)
    warn.mockRestore()
  })

  it('a rejected detection keeps the last known accounts too', async () => {
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshClaudeAccounts()

    listClaudeAccounts.mockRejectedValue(new Error('daemon down'))
    await refreshClaudeAccounts({ force: true })

    expect(claudeAccounts.accounts).toHaveLength(2)
    expect(claudeAccounts.degraded).toBe(true)
  })

  // Regression: c982822 cached a failed detection as an empty account list for
  // the full 60 s TTL, so a daemon that connected a moment later could not
  // restore the chooser.
  it('detects again after a failure instead of caching the empty list', async () => {
    listClaudeAccounts.mockRejectedValueOnce(new Error('daemon down'))
    await refreshClaudeAccounts()
    listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

    await requestClaudeLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  describe('an account picked in the context menu', () => {
    // Regression: c982822 made the chooser the only way to name an account, so
    // the sidebar's launch items could not carry the answer the user had
    // already given by picking a row. An explicit id is the decision itself:
    // it must launch, not reopen the question.
    it('launches on it without opening the chooser', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshClaudeAccounts()

      await requestClaudeLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    // Regression: 74c7761 pinned the project to whatever a launch row named
    // when the project had chosen nothing. A pin is written by the chooser's
    // remember, the chip, and the Account submenu — a row picked for one launch
    // is one launch, and it must not move every later launch with it.
    it('pins nothing: the row is this launch, not the project default', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshClaudeAccounts()

      await requestClaudeLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(setProjectClaudeAccount).not.toHaveBeenCalled()
      expect(effectiveClaudeAccountId({ id: 'p1' })).toBe(null)
      expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    it('leaves a project that already chose alone — one launch is not a new pin', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshClaudeAccounts()

      await requestClaudeLaunch({
        project: { id: 'p1', claude_account_id: 'account-1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(setProjectClaudeAccount).not.toHaveBeenCalled()
      expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    it('starts the launch without waiting for a database write', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshClaudeAccounts()
      setProjectClaudeAccount.mockImplementation(() => new Promise(() => {}))

      await requestClaudeLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(launchClaudeSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })
  })

  describe('the account a launch would use today', () => {
    it('follows pin, then global default, then the default config dir', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshClaudeAccounts()

      expect(activeClaudeAccountId({ id: 'p1' })).toBe('account-1')

      setGlobalClaudeAccount('account-2')
      expect(activeClaudeAccountId({ id: 'p1' })).toBe('account-2')

      expect(activeClaudeAccountId({ id: 'p1', claude_account_id: 'account-1' })).toBe('account-1')
    })

    it('ignores a pin whose account cannot run', async () => {
      listClaudeAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }]))
      await refreshClaudeAccounts()

      expect(activeClaudeAccountId({ id: 'p1', claude_account_id: 'account-2' })).toBe('account-1')
    })
  })
})
