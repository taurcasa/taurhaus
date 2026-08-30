import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ipc.js', () => ({
  refreshAccountsUsage: vi.fn(() => Promise.resolve(true)),
  listAccounts: vi.fn(),
  setProjectAccount: vi.fn(),
  launchCliSession: vi.fn(),
  resolveLaunchAccount: vi.fn(),
  resolveLaunchBases: vi.fn(() => Promise.resolve([])),
  getSettings: vi.fn(),
}))

const {
  listAccounts,
  refreshAccountsUsage,
  setProjectAccount,
  launchCliSession,
  resolveLaunchAccount,
  resolveLaunchBases,
  getSettings,
} = await import('./ipc.js')
const {
  accountState,
  activeAccountId,
  effectiveAccount,
  loggedInAccounts,
  refreshAccounts,
  refreshResolvedBases,
  refreshUsage,
  requestLaunch,
  resolveChooserAccounts,
  resetAccountsForTest,
  setDefaultAccount,
} = await import('./accounts.svelte.js')

const claudeAccounts = accountState('claude')
function effectiveClaudeAccountId(project) {
  if (project?.id && project.id in claudeAccounts.projectChoices) {
    return claudeAccounts.projectChoices[project.id]
  }
  return project?.accountMemory?.claude?.accountId ?? null
}

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
    resetAccountsForTest()
    setProjectAccount.mockResolvedValue(undefined)
    launchCliSession.mockResolvedValue({ tmux_pane: '%1' })
    resolveLaunchAccount.mockResolvedValue({
      accountId: null,
      source: 'default_config_dir',
      needsChoice: true,
    })
    resolveLaunchBases.mockResolvedValue([])
    getSettings.mockResolvedValue({ terminal: { default_account_ids: {} } })
  })

  // Regression: b1856a33 cached a successful fail-soft launch-base response
  // forever. If the daemon was down for that call, Settings pinned the literal
  // command for the rest of the app run instead of asking again after a minute.
  it('re-asks for launch bases after the detection TTL', async () => {
    vi.useFakeTimers()
    try {
      resolveLaunchBases
        .mockResolvedValueOnce([{ command: 'claude2 --fresh' }])
        .mockResolvedValueOnce([{ command: 'claude --fresh' }])

      await refreshResolvedBases('claude')
      expect(resolveLaunchBases).toHaveBeenCalledTimes(1)

      await vi.advanceTimersByTimeAsync(60_001)
      await refreshResolvedBases('claude')

      expect(resolveLaunchBases).toHaveBeenCalledTimes(2)
      expect(claudeAccounts.resolvedBases).toEqual([{ command: 'claude --fresh' }])
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
  })

  // Regression: 3c5b6cd9 invalidated only the app process's launch-base cache.
  // On Windows the answer lives in the WSL daemon, so a Settings save replayed
  // its stale or failed answer until the daemon's ten-minute TTL elapsed.
  it('forwards a forced launch-base refresh to the backend', async () => {
    await refreshResolvedBases('claude', { force: true })

    expect(resolveLaunchBases).toHaveBeenCalledWith('claude', true)
  })

  it('keeps a logged-out account visible for the chooser but out of the count', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }]))

    await refreshAccounts('claude')

    expect(claudeAccounts.accounts).toHaveLength(2)
    expect(resolveChooserAccounts('claude').map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(loggedInAccounts('claude')).toHaveLength(1)
  })

  // Regression: 6ec843e kept one row per detected config dir, and one
  // subscription signed into two of them is detected twice under the same
  // account uuid. That uuid is the only address a launch or a pin has, so the
  // `.claude-account2` row launched `.claude`, both rows read as the current
  // one, and the chip and chooser — which key their rows by that id — threw
  // `each_key_duplicate` on the pair.
  it('keeps one account per id a launch can name', async () => {
    const sameSubscriptionElsewhere = {
      ...PRIMARY,
      is_default: false,
      config_dir: '/home/user/.claude-copy',
    }
    listAccounts.mockResolvedValue(detected([PRIMARY, sameSubscriptionElsewhere, SECOND]))

    await refreshAccounts('claude')

    expect(resolveChooserAccounts('claude').map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(resolveChooserAccounts('claude')[0].config_dir).toBe('/home/user/.claude')
  })

  // The kept row has to be the one the backend resolves that id to, which is
  // the first that can actually run — not simply the first seen.
  it('keeps the signed-in dir when the default dir of the same account is not', async () => {
    listAccounts.mockResolvedValue(
      detected([
        { ...PRIMARY, logged_in: false },
        { ...PRIMARY, is_default: false, config_dir: '/home/user/.claude-copy' },
      ])
    )

    await refreshAccounts('claude')

    expect(resolveChooserAccounts('claude')).toHaveLength(1)
    expect(resolveChooserAccounts('claude')[0].config_dir).toBe('/home/user/.claude-copy')
    expect(loggedInAccounts('claude')).toHaveLength(1)
  })

  it('launches straight away when only one account is logged in', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY]))
    await refreshAccounts('claude')

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('asks for a Codex account through the same generic store', async () => {
    // Regression: 08c3961 left Codex account selection disabled after the
    // generic chooser state landed, so two CODEX_HOME accounts were ignored.
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('codex')

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'codex' })

    expect(accountState('codex').pending).toMatchObject({
      projectId: 'p1',
      mode: 'fresh',
      tool: 'codex',
    })
    expect(launchCliSession).not.toHaveBeenCalled()
  })

  it('asks once when two accounts are logged in and the project stored no choice', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1', mode: 'fresh' })
  })

  it('remembers the choice for the project and launches on it', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')
    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', true)

    expect(setProjectAccount).toHaveBeenCalledWith('p1', 'claude', 'account-2')
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    expect(claudeAccounts.pending).toBe(null)
  })

  it('does not store the choice when the user unticks remember', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')
    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', false)

    expect(setProjectAccount).not.toHaveBeenCalled()
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
  })

  it('skips the chooser once the project has a stored account', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    await requestLaunch({
      project: { id: 'p1', accountMemory: { claude: { accountId: 'account-2', origin: 'pinned' } } },
      mode: 'continue',
      tool: 'claude',
    })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'continue', 'claude', null)
  })

  it('cancelling the chooser launches nothing', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')
    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    claudeAccounts.pending.cancel()

    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toBe(null)
  })

  it('a detection failure never blocks the launch', async () => {
    listAccounts.mockRejectedValue(new Error('daemon down'))
    await refreshAccounts('claude')

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  // Regression: c982822 decided whether to ask from the project object the
  // caller happened to hold and updated nothing after storing the answer, so
  // the same project asked again on every launch of the app session.
  it('asks once and never again once the choice is remembered', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    const project = { id: 'p1' }
    await requestLaunch({ project, mode: 'fresh', tool: 'claude' })
    await claudeAccounts.pending.confirm('account-2', true)

    await requestLaunch({ project, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchCliSession).toHaveBeenCalledTimes(2)
    expect(effectiveClaudeAccountId(project)).toBe('account-2')
  })

  it('a failed store drops the remembered choice again', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    setProjectAccount.mockRejectedValue(new Error('db locked'))
    const project = { id: 'p1' }
    await requestLaunch({ project, mode: 'fresh', tool: 'claude' })

    await claudeAccounts.pending.confirm('account-2', true)

    expect(effectiveClaudeAccountId(project)).toBe(null)
  })

  // Regression: c982822 left the configured global default to the backend
  // alone. The chooser asked anyway, and Enter pinned the project to whichever
  // account happened to sit in the default config dir.
  it('never asks when a global default account is configured', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    getSettings.mockResolvedValue({ terminal: { default_account_ids: { claude: 'account-2' } } })

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(claudeAccounts.defaultAccountId).toBe('account-2')
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
  })

  it('asks again when the configured global default is no longer usable', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }, THIRD]))
    getSettings.mockResolvedValue({ terminal: { default_account_ids: { claude: 'account-2' } } })

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: 79be608 folded usage into account detection, which the store
  // caches for a minute and OverviewTab asked for once on mount. Every later
  // status-line report was invisible until something forced detection again,
  // so the chip a user opened to compare subscriptions showed mount-time
  // numbers — for a feature whose whole point is the current one.
  it('refreshes usage without waiting for the detection cache to expire', async () => {
    listAccounts.mockResolvedValue(
      detected([PRIMARY, { ...SECOND, usage: null }])
    )
    await refreshAccounts('claude')
    expect(claudeAccounts.accounts[1].usage).toBe(null)

    const usage = {
      five_hour: { used_percentage: 81, resets_at: 1787784600 },
      seven_day: { used_percentage: 44, resets_at: 1788300000 },
      observed_at: '2026-08-27T12:00:00Z',
    }
    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage }]))

    await refreshUsage('claude')

    expect(claudeAccounts.accounts[1].usage).toEqual(usage)
    // The accounts themselves are detection's answer, not this call's.
    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
  })

  it('re-reads usage after an asynchronous refresh is acknowledged', async () => {
    // Regression: 2f8246c made the refresh RPC wait for network completion.
    // Returning promptly avoids daemon disconnects, so the frontend must keep
    // reading until the poller's newer observation is visible.
    vi.useFakeTimers()
    try {
      const previousUsage = { observed_at: '2026-08-27T12:00:00Z', windows: [] }
      const refreshedUsage = { observed_at: '2026-08-27T12:00:01Z', windows: [] }
      const unsupported = { ...PRIMARY, usage_capable: false }
      const previousReport = detected([unsupported, { ...SECOND, usage: previousUsage }])
      const refreshedReport = detected([unsupported, { ...SECOND, usage: refreshedUsage }])
      listAccounts.mockResolvedValue(previousReport)
      await refreshAccounts('claude')
      listAccounts.mockResolvedValueOnce(previousReport).mockResolvedValue(refreshedReport)

      await refreshUsage('claude')
      expect(claudeAccounts.accounts[1].usage).toEqual(previousUsage)

      await vi.advanceTimersByTimeAsync(250)

      expect(claudeAccounts.accounts[1].usage).toEqual(refreshedUsage)
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
  })

  it('publishes an account first usage snapshot after an asynchronous refresh', async () => {
    // Regression: c71cedb stopped the refresh retry chain for accounts without
    // an existing observation, so their first successful background fetch was
    // never read and a newly opened chooser stayed meterless.
    vi.useFakeTimers()
    try {
      const emptyReport = detected([{ ...PRIMARY, usage: null }])
      const firstUsage = { observed_at: '2026-08-27T12:00:00Z', windows: [] }
      const firstReport = detected([{ ...PRIMARY, usage: firstUsage }])
      listAccounts.mockResolvedValue(emptyReport)
      await refreshAccounts('claude')
      listAccounts.mockResolvedValueOnce(emptyReport).mockResolvedValue(firstReport)

      await refreshUsage('claude')
      expect(claudeAccounts.accounts[0].usage).toBeNull()

      await vi.advanceTimersByTimeAsync(250)

      expect(claudeAccounts.accounts[0].usage).toEqual(firstUsage)
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
  })

  it('skips usage sync for accounts the provider marks unsupported', async () => {
    // An account without subscription usage must not keep another account's
    // refresh chain alive after that observable snapshot advances.
    vi.useFakeTimers()
    try {
      const previousUsage = { observed_at: '2026-08-27T12:00:00Z', windows: [] }
      const refreshedUsage = { observed_at: '2026-08-27T12:00:01Z', windows: [] }
      const unsupported = { ...PRIMARY, usage_capable: false }
      const previousReport = detected([unsupported, { ...SECOND, usage: previousUsage }])
      const refreshedReport = detected([unsupported, { ...SECOND, usage: refreshedUsage }])
      listAccounts.mockResolvedValue(previousReport)
      await refreshAccounts('claude')
      listAccounts.mockResolvedValueOnce(previousReport).mockResolvedValue(refreshedReport)

      await refreshUsage('claude')
      await vi.advanceTimersByTimeAsync(250)
      expect(claudeAccounts.accounts[1].usage).toEqual(refreshedUsage)

      await vi.advanceTimersByTimeAsync(30_000)

      expect(listAccounts).toHaveBeenCalledTimes(3)
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
  })

  it('backs off while a usage-capable account waits for its first snapshot', async () => {
    // Regression: 701cd7c polled list_accounts at 4 Hz for the full refresh
    // deadline when a usage-capable account had not published a snapshot yet.
    vi.useFakeTimers()
    try {
      const report = detected([{ ...PRIMARY, usage: null }])
      listAccounts.mockResolvedValue(report)
      await refreshAccounts('claude')

      await refreshUsage('claude')
      await vi.advanceTimersByTimeAsync(30_000)

      expect(listAccounts.mock.calls.length).toBeLessThanOrEqual(8)
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
  })

  it('backs repeated usage reads off geometrically', async () => {
    // Regression: 701cd7c retried list_accounts at a fixed 250 ms cadence for
    // up to 30 seconds while the usage poller was still producing a snapshot.
    vi.useFakeTimers()
    try {
      const usage = { observed_at: '2026-08-27T12:00:00Z', windows: [] }
      const report = detected([{ ...PRIMARY, usage }])
      listAccounts.mockResolvedValue(report)
      await refreshAccounts('claude')
      await refreshUsage('claude')

      await vi.advanceTimersByTimeAsync(250)
      expect(listAccounts).toHaveBeenCalledTimes(3)

      await vi.advanceTimersByTimeAsync(250)
      expect(listAccounts).toHaveBeenCalledTimes(3)

      await vi.advanceTimersByTimeAsync(250)
      expect(listAccounts).toHaveBeenCalledTimes(4)

      await vi.advanceTimersByTimeAsync(999)
      expect(listAccounts).toHaveBeenCalledTimes(4)

      await vi.advanceTimersByTimeAsync(1)
      expect(listAccounts).toHaveBeenCalledTimes(5)
    } finally {
      resetAccountsForTest()
      vi.useRealTimers()
    }
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
    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage }]))
    await refreshAccounts('claude')
    expect(claudeAccounts.accounts[1].usage).toEqual(usage)

    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, usage: null }]))
    await refreshUsage('claude')

    expect(claudeAccounts.accounts[1].usage).toEqual(usage)

    // Detection answers with the same numbers from the same file, so it keeps
    // them for the same reason.
    await refreshAccounts('claude', { force: true })
    expect(claudeAccounts.accounts[1].usage).toEqual(usage)
  })

  it('keeps the accounts it knows when a usage refresh cannot run', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    listAccounts.mockResolvedValue(degraded())
    await refreshUsage('claude')

    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
  })

  it('reads usage again before asking which subscription to launch on', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')
    listAccounts.mockClear()

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
    expect(listAccounts).toHaveBeenCalled()
  })

  it('a default chosen in settings takes effect without a reload', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    setDefaultAccount('claude', 'account-2')

    expect(claudeAccounts.defaultAccountId).toBe('account-2')
    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })
    expect(claudeAccounts.pending).toBe(null)
  })

  // Regression: c982822 read the account list synchronously while detection
  // was still in flight, so a launch clicked during startup silently ran on
  // the backend default instead of asking.
  it('waits for detection that is still in flight', async () => {
    let publish
    listAccounts.mockReturnValue(
      new Promise((resolve) => {
        publish = resolve
      })
    )
    const detecting = refreshAccounts('claude')

    const launching = requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })
    publish(detected([PRIMARY, SECOND]))
    await detecting
    await launching

    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: c982822 applied the chooser to every mode. `--resume` only
  // sees the history of the config dir it runs in, and the backend derives
  // that dir from the project's last transcript — but an explicit answer from
  // the chooser outranks it, so a resume was pinned to whichever account the
  // user picked in a dialog that should never have opened.
  it('never asks for a resume the backend can already place', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    resolveLaunchAccount.mockResolvedValue({
      accountId: 'account-2',
      source: 'session',
      needsChoice: false,
    })

    await requestLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

    expect(claudeAccounts.pending).toBe(null)
    expect(launchCliSession).toHaveBeenCalledWith('p1', 'resume', 'claude', null)
  })

  it('asks for a resume the backend cannot place', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    resolveLaunchAccount.mockResolvedValue({
      accountId: null,
      source: 'default_config_dir',
      needsChoice: true,
    })

    await requestLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1', mode: 'resume' })
  })

  // Regression: c982822 treated any stored project account id as an answer.
  // A pinned account that logged out is not one: the backend refuses it, and
  // with the default config dir signed out too the launch landed on an account
  // nobody chose while several usable ones waited.
  it('asks again when the stored project account can no longer run', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }, THIRD]))

    await requestLaunch({
      project: { id: 'p1', accountMemory: { claude: { accountId: 'account-2', origin: 'pinned' } } },
      mode: 'fresh',
      tool: 'claude',
    })

    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  // Regression: 518aace let the backend answer every daemon failure with a
  // successful empty list. The store believed it: the chooser stopped asking,
  // the chip disappeared, and launches ran on whichever subscription the
  // default config dir happened to hold — while both accounts were still
  // signed in and nothing on screen said otherwise.
  it('keeps the accounts it last knew when detection is degraded', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    listAccounts.mockResolvedValue(degraded())
    await refreshAccounts('claude', { force: true })

    expect(claudeAccounts.accounts.map((account) => account.id)).toEqual([
      'account-1',
      'account-2',
    ])
    expect(claudeAccounts.degraded).toBe(true)
  })

  it('still asks which subscription to use while detection is degraded', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')
    listAccounts.mockResolvedValue(degraded())
    await refreshAccounts('claude', { force: true })

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(launchCliSession).not.toHaveBeenCalled()
    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  it('detects again after a degraded answer instead of caching it', async () => {
    listAccounts.mockResolvedValueOnce(degraded())
    await refreshAccounts('claude')
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

    await refreshAccounts('claude')

    expect(claudeAccounts.degraded).toBe(false)
    expect(claudeAccounts.accounts).toHaveLength(2)
  })

  // Regression: 74c7761 asked for detection on every context-menu opening, and
  // a degraded answer warned every time. Every warn crosses the logging bridge,
  // so right-clicking during a daemon outage wrote an unbounded stream of the
  // same line. The outage is one event; the recovery is the next one.
  it('warns once while detection stays degraded, and again when it returns', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    listAccounts.mockResolvedValue(degraded())

    await refreshAccounts('claude')
    await refreshAccounts('claude', { force: true })
    await refreshAccounts('claude', { force: true })

    const degradedWarnings = () =>
      warn.mock.calls.filter(([message]) =>
        String(message).includes('Account detection is unavailable')
      ).length
    expect(degradedWarnings()).toBe(1)

    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude', { force: true })
    listAccounts.mockResolvedValue(degraded())
    await refreshAccounts('claude', { force: true })

    expect(degradedWarnings()).toBe(2)
    warn.mockRestore()
  })

  it('a rejected detection keeps the last known accounts too', async () => {
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
    await refreshAccounts('claude')

    listAccounts.mockRejectedValue(new Error('daemon down'))
    await refreshAccounts('claude', { force: true })

    expect(claudeAccounts.accounts).toHaveLength(2)
    expect(claudeAccounts.degraded).toBe(true)
  })

  // Regression: c982822 cached a failed detection as an empty account list for
  // the full 60 s TTL, so a daemon that connected a moment later could not
  // restore the chooser.
  it('detects again after a failure instead of caching the empty list', async () => {
    listAccounts.mockRejectedValueOnce(new Error('daemon down'))
    await refreshAccounts('claude')
    listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

    await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

    expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
  })

  describe('an account picked in the context menu', () => {
    // Regression: c982822 made the chooser the only way to name an account, so
    // the sidebar's launch items could not carry the answer the user had
    // already given by picking a row. An explicit id is the decision itself:
    // it must launch, not reopen the question.
    it('launches on it without opening the chooser', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')

      await requestLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    // Regression: 74c7761 pinned the project to whatever a launch row named
    // when the project had chosen nothing. A pin is written by the chooser's
    // remember, the chip, and the Account submenu — a row picked for one launch
    // is one launch, and it must not move every later launch with it.
    it('pins nothing: the row is this launch, not the project default', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')

      await requestLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(setProjectAccount).not.toHaveBeenCalled()
      expect(effectiveClaudeAccountId({ id: 'p1' })).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    it('leaves a project that already chose alone — one launch is not a new pin', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')

      await requestLaunch({
        project: { id: 'p1', accountMemory: { claude: { accountId: 'account-1', origin: 'pinned' } } },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(setProjectAccount).not.toHaveBeenCalled()
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })

    it('starts the launch without waiting for a database write', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')
      setProjectAccount.mockImplementation(() => new Promise(() => {}))

      await requestLaunch({
        project: { id: 'p1' },
        mode: 'fresh',
        tool: 'claude',
        accountId: 'account-2',
      })

      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
    })
  })

  describe('the chooser as the way out of a spent subscription', () => {
    const OBSERVED_AT = '2026-08-30T08:00:00Z'
    // Anchored to the run's own clock: a limit that has already come back is
    // not a spent limit, and a fixture with a fixed reset would quietly become
    // one as the calendar moved past it.
    const RESETS_AT = Math.floor(Date.now() / 1000) + 7 * 24 * 60 * 60
    const ALREADY_RESET = Math.floor(Date.now() / 1000) - 60

    const windowAt = (key, title, used, extra = {}) => ({
      key,
      title,
      used_percentage: used,
      resets_at: RESETS_AT,
      severity: 'normal',
      is_active: true,
      ...extra,
    })
    const snapshot = (status, windows) => ({
      observed_at: OBSERVED_AT,
      status,
      windows,
      note: null,
    })
    const SPENT = snapshot('ok', [
      windowAt('session', 'Current session', 12),
      windowAt('week', 'Current week (all models)', 100, { severity: 'critical' }),
    ])
    const HEADROOM = snapshot('ok', [
      windowAt('session', 'Current session', 12),
      windowAt('week', 'Current week (all models)', 44),
    ])
    const remembering = (accountId = 'account-1', origin = 'last_used') => ({
      id: 'p1',
      accountMemory: { claude: { accountId, origin } },
    })
    /** What detection publishes, with one account carrying a reading. */
    const knowing = (usage, accountId = 'account-1') =>
      detected(
        [PRIMARY, SECOND].map((account) =>
          account.id === accountId ? { ...account, usage } : account
        )
      )

    // Regression: #35 (per-project account memory) made the chooser open only
    // when nothing had decided the launch. Every project that has ever launched
    // remembers an account, so the dialog never appeared again — including at
    // the one moment its answer matters, when the remembered subscription has
    // run out of usage and the launch would silently continue into it.
    it('opens on the remembered account being spent, instead of launching into it', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(launchCliSession).not.toHaveBeenCalled()
      expect(claudeAccounts.pending).toMatchObject({
        projectId: 'p1',
        reason: {
          kind: 'exhausted',
          accountLabel: 'stierms@gmail.com',
          windowTitle: 'Current week (all models)',
          resetsAt: RESETS_AT,
        },
      })
    })

    /*
     * Regression: the six majors of review rounds 2–4, all of them holes in one
     * shape — "ask for a refresh, wait for the reading it publishes, then
     * judge". A debounced refresh judged the numbers the fetch it was debounced
     * against was about to replace (2bec263); reading the Windows daemon's
     * reply as a started fetch stalled every remembered launch for the full
     * 30-second deadline (c11770e); a refresh the daemon could not be asked at
     * all read as "these numbers are current" (4011b02); an account already
     * found signed out waited out the whole deadline because the poller
     * republishes nothing until its credential file changes; a transient
     * failure stayed indistinguishable from a debounce; and every later
     * refresh pushed the shared deadline out again, so an ordinary hover could
     * hold a launch open indefinitely.
     *
     * The waiting is gone: a weekly or five-hour limit is slow-moving state,
     * the reading `refreshAccounts` just returned is the one the chip and the
     * menus already show, and the chooser stays reachable on demand. So the
     * launch is judged on the reading in hand and a fresher one is only asked
     * for in the background.
     */
    it('decides on the reading in hand, without waiting for the refresh it asks for', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      // The backend never answers this one: nothing may depend on it.
      refreshAccountsUsage.mockReturnValueOnce(new Promise(() => {}))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(launchCliSession).not.toHaveBeenCalled()
      expect(claudeAccounts.pending).toMatchObject({ reason: { kind: 'exhausted' } })
    })

    it('asks for a fresher reading in the background, for the next launch and the open dialog', async () => {
      listAccounts.mockResolvedValue(knowing(HEADROOM))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(refreshAccountsUsage).toHaveBeenCalledWith('claude')
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('launches on the remembered account while it still has headroom', async () => {
      listAccounts.mockResolvedValue(knowing(HEADROOM))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('launches when nothing has reported usage for the remembered account', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('says an account that cannot be read needs signing in again, at once', async () => {
      listAccounts.mockResolvedValue(knowing(snapshot('unauthorized', [])))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(launchCliSession).not.toHaveBeenCalled()
      expect(claudeAccounts.pending).toMatchObject({
        reason: { kind: 'unauthorized', accountLabel: 'stierms@gmail.com' },
      })
    })

    it('counts a stale reading — it is the last thing known about the limit', async () => {
      listAccounts.mockResolvedValue(
        knowing(snapshot('stale', [windowAt('week', 'Current week', 100)]))
      )

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(claudeAccounts.pending).toMatchObject({ reason: { kind: 'exhausted' } })
    })

    it('launches on a limit that has already come back, however old the reading is', async () => {
      listAccounts.mockResolvedValue(
        knowing(
          snapshot('stale', [
            windowAt('week', 'Current week', 100, { resets_at: ALREADY_RESET }),
          ])
        )
      )

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('launches while detection is degraded rather than judging what nothing confirmed', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      await refreshAccounts('claude')
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
      listAccounts.mockResolvedValue(degraded())
      await refreshAccounts('claude', { force: true })

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      warn.mockRestore()
      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('never blocks a launch on an account nothing was ever known about', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
      listAccounts.mockRejectedValue(new Error('daemon down'))

      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      warn.mockRestore()
      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
    })

    it('checks the account a resume the backend places would run on', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      resolveLaunchAccount.mockResolvedValue({
        accountId: 'account-1',
        source: 'session',
        needsChoice: false,
      })

      await requestLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

      expect(launchCliSession).not.toHaveBeenCalled()
      expect(claudeAccounts.pending).toMatchObject({
        mode: 'resume',
        reason: { kind: 'exhausted' },
      })
    })

    it('still asks nothing for a resume whose account has headroom', async () => {
      listAccounts.mockResolvedValue(knowing(HEADROOM))
      resolveLaunchAccount.mockResolvedValue({
        accountId: 'account-1',
        source: 'session',
        needsChoice: false,
      })

      await requestLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'resume', 'claude', null)
    })

    // Regression: ea6dca0 asked the backend only whether *something* had placed
    // the launch and then read the usage of whichever account the frontend's
    // own precedence landed on. The transcript that decides a resume is the
    // backend's to read, and it answers with the account itself: here it places
    // the resume on the second subscription while the frontend would have
    // judged the first, so the spent one was never seen.
    it('checks the account the backend places the resume on, not the frontend fallback', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT, 'account-2'))
      resolveLaunchAccount.mockResolvedValue({
        accountId: 'account-2',
        source: 'session',
        needsChoice: false,
      })

      await requestLaunch({ project: { id: 'p1' }, mode: 'resume', tool: 'claude' })

      expect(launchCliSession).not.toHaveBeenCalled()
      expect(claudeAccounts.pending).toMatchObject({
        mode: 'resume',
        reason: { kind: 'exhausted', accountLabel: 'm.stier@giesi.com' },
      })
    })

    // Regression: ea6dca0 — the same confusion in the other direction: the
    // project's memory named a spent account the resume would never have run
    // on, and the chooser opened over a transcript the backend had already
    // placed on a subscription with headroom.
    it('launches a resume the backend places on an account with headroom, whatever the project remembers', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      resolveLaunchAccount.mockResolvedValue({
        accountId: 'account-2',
        source: 'session',
        needsChoice: false,
      })

      await requestLaunch({ project: remembering(), mode: 'resume', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'resume', 'claude', null)
    })

    // Regression: ea6dca0 — an account the frontend cannot name is not one it
    // may judge in another account's place.
    it('launches when the backend places the resume on an account nothing here knows', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      resolveLaunchAccount.mockResolvedValue({
        accountId: 'account-9',
        source: 'session',
        needsChoice: false,
      })

      await requestLaunch({ project: remembering(), mode: 'resume', tool: 'claude' })

      expect(claudeAccounts.pending).toBe(null)
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'resume', 'claude', null)
    })

    it('carries no reason when nothing had decided the account anyway', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))

      await requestLaunch({ project: { id: 'p1' }, mode: 'fresh', tool: 'claude' })

      expect(claudeAccounts.pending).toMatchObject({ projectId: 'p1' })
      expect(claudeAccounts.pending.reason).toBe(null)
    })

    it('confirming a reasoned chooser still pins and launches', async () => {
      listAccounts.mockResolvedValue(knowing(SPENT))
      await requestLaunch({ project: remembering(), mode: 'fresh', tool: 'claude' })

      await claudeAccounts.pending.confirm('account-2', true)

      expect(setProjectAccount).toHaveBeenCalledWith('p1', 'claude', 'account-2')
      expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
      expect(claudeAccounts.pending).toBe(null)
    })

    describe("choose: 'always'", () => {
      it('opens the chooser on a project that already remembers an account', async () => {
        listAccounts.mockResolvedValue(knowing(HEADROOM))

        await requestLaunch({
          project: remembering('account-2', 'pinned'),
          mode: 'fresh',
          tool: 'claude',
          choose: 'always',
        })

        expect(launchCliSession).not.toHaveBeenCalled()
        expect(claudeAccounts.pending).toMatchObject({
          projectId: 'p1',
          preselectedAccountId: 'account-2',
        })
        expect(claudeAccounts.pending.reason).toBe(null)
      })

      it('asks the backend nothing: the user has already said they want to choose', async () => {
        listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
        resolveLaunchAccount.mockResolvedValue({ needsChoice: false, accountId: 'account-1' })

        await requestLaunch({
          project: { id: 'p1' },
          mode: 'resume',
          tool: 'claude',
          choose: 'always',
        })

        expect(resolveLaunchAccount).not.toHaveBeenCalled()
        expect(claudeAccounts.pending).toMatchObject({ mode: 'resume' })
      })

      it('reads usage before offering the comparison', async () => {
        listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
        await refreshAccounts('claude')
        refreshAccountsUsage.mockClear()

        await requestLaunch({
          project: remembering(),
          mode: 'fresh',
          tool: 'claude',
          choose: 'always',
        })

        expect(refreshAccountsUsage).toHaveBeenCalledWith('claude')
      })

      it('does not chase a newer reading: the dialog is already open', async () => {
        // The meters fill in as the poller publishes; a user who asked to
        // choose is looking at the list, not at a launch that has stopped.
        listAccounts.mockResolvedValue(knowing(HEADROOM))

        await requestLaunch({
          project: remembering(),
          mode: 'fresh',
          tool: 'claude',
          choose: 'always',
        })

        expect(claudeAccounts.pending).toMatchObject({ preselectedAccountId: 'account-1' })
        expect(listAccounts).toHaveBeenCalledTimes(2)
      })

      it('is still outranked by an account the caller named outright', async () => {
        listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))

        await requestLaunch({
          project: remembering(),
          mode: 'fresh',
          tool: 'claude',
          accountId: 'account-2',
          choose: 'always',
        })

        expect(claudeAccounts.pending).toBe(null)
        expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', 'account-2')
      })

      it('has nothing to ask when only one account can run', async () => {
        listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }]))

        await requestLaunch({
          project: remembering(),
          mode: 'fresh',
          tool: 'claude',
          choose: 'always',
        })

        expect(claudeAccounts.pending).toBe(null)
        expect(launchCliSession).toHaveBeenCalledWith('p1', 'fresh', 'claude', null)
      })
    })
  })

  describe('the account a launch would use today', () => {
    it('applies every generic precedence slice in order', () => {
      const state = accountState('claude')
      state.accounts = [
        { ...PRIMARY, is_process_default: true },
        SECOND,
        THIRD,
      ]
      const project = {
        id: 'p1',
        explicitAccountIds: { claude: 'account-3' },
        sessionAccountIds: { claude: 'account-2' },
        accountMemory: { claude: { accountId: 'account-1', origin: 'pinned' } },
        baseCommandAccountIds: { claude: 'account-2' },
      }

      expect(effectiveAccount(project, 'claude').origin).toBe('explicit')
      delete project.explicitAccountIds
      expect(effectiveAccount(project, 'claude').origin).toBe('session')
      delete project.sessionAccountIds
      expect(effectiveAccount(project, 'claude').origin).toBe('pinned')
      project.accountMemory.claude = { accountId: 'account-3', origin: 'last_used' }
      expect(effectiveAccount(project, 'claude').origin).toBe('last_used')
      delete project.accountMemory
      state.defaultAccountId = 'account-3'
      expect(effectiveAccount(project, 'claude').origin).toBe('default')
      state.defaultAccountId = null
      expect(effectiveAccount(project, 'claude').origin).toBe('base_command')
      delete project.baseCommandAccountIds
      expect(effectiveAccount(project, 'claude').origin).toBe('default_config_dir')
    })

    it('follows pin, then global default, then the default config dir', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, SECOND]))
      await refreshAccounts('claude')

      expect(activeAccountId({ id: 'p1' }, 'claude')).toBe('account-1')

      setDefaultAccount('claude', 'account-2')
      expect(activeAccountId({ id: 'p1' }, 'claude')).toBe('account-2')

      expect(activeAccountId({ id: 'p1', accountMemory: { claude: { accountId: 'account-1', origin: 'pinned' } } }, 'claude')).toBe('account-1')
    })

    it('ignores a pin whose account cannot run', async () => {
      listAccounts.mockResolvedValue(detected([PRIMARY, { ...SECOND, logged_in: false }]))
      await refreshAccounts('claude')

      expect(activeAccountId({ id: 'p1', accountMemory: { claude: { accountId: 'account-2', origin: 'pinned' } } }, 'claude')).toBe('account-1')
    })
  })
})
