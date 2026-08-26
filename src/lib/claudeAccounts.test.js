import { beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('./ipc.js', () => ({
  listClaudeAccounts: vi.fn(),
  setProjectClaudeAccount: vi.fn(),
  launchClaudeSession: vi.fn(),
}))

const { listClaudeAccounts, setProjectClaudeAccount, launchClaudeSession } = await import('./ipc.js')
const {
  claudeAccounts,
  loggedInAccounts,
  refreshClaudeAccounts,
  requestClaudeLaunch,
  resolveChooserAccounts,
  resetClaudeAccountsForTest,
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

describe('claudeAccounts store', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    resetClaudeAccountsForTest()
    setProjectClaudeAccount.mockResolvedValue(undefined)
    launchClaudeSession.mockResolvedValue({ tmux_pane: '%1' })
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
})
