import { beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../accounts.svelte.js', () => ({
  accountState: vi.fn(() => ({ accounts: [], relationships: {}, resolvedBases: [] })),
  opaqueBaseNotice: (head) =>
    `taurhaus could not select an account: your launch command runs "${head}", which is not the Claude CLI`,
  refreshAccounts: vi.fn(() => Promise.resolve()),
  refreshAccountRelationships: vi.fn(() => Promise.resolve()),
  refreshResolvedBases: vi.fn(() => Promise.resolve()),
  refreshUsage: vi.fn(() => Promise.resolve()),
  rememberChoice: vi.fn(() => Promise.resolve()),
  setGlobalDefault: vi.fn(() => Promise.resolve()),
}))

vi.mock('../ipc.js', () => ({
  revealDirectory: vi.fn(() => Promise.resolve()),
  prepareAccountDirectory: vi.fn(() => Promise.resolve()),
  launchAccountLogin: vi.fn(() => Promise.resolve()),
}))

import AccountsHome from './AccountsHome.svelte'
import {
  refreshAccounts,
  refreshUsage,
  rememberChoice,
  setGlobalDefault,
} from '../accounts.svelte.js'
import { revealDirectory } from '../ipc.js'

const now = Date.now()
const usage = (used = 42) => ({
  status: 'ok',
  observed_at: new Date(now - 2 * 60_000).toISOString(),
  windows: [
    {
      key: 'weekly',
      title: 'Current week',
      used_percentage: used,
      resets_at: Math.floor(now / 1000) + 86_400,
      severity: used >= 100 ? 'critical' : 'normal',
    },
  ],
})

const account = (id, overrides = {}) => ({
  id,
  label: `${id}@example.com`,
  display_name: id,
  dir: `/home/user/.claude-${id}`,
  logged_in: true,
  usage: usage(),
  ...overrides,
})

function states() {
  return {
    claude: {
      accounts: [account('personal'), account('work', { usage: usage(100) })],
      defaultAccountId: 'personal',
      degraded: false,
      relationships: {
        work: {
          pinnedProjects: [{ id: 'p1', name: 'taurhaus', path: '/work/taurhaus' }],
          lastUsedProjects: [{ id: 'p2', name: 'mir', path: '/work/mir' }],
          teams: [{ name: 'wave-a', projectId: 'p1', projectName: 'taurhaus' }],
        },
      },
      resolvedBases: [
        {
          command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
          selectorValue: '/home/user/.claude-work',
          expansions: [{ name: 'claude2', body: 'CLAUDE_CONFIG_DIR=…' }],
        },
      ],
    },
    codex: {
      accounts: [account('codex', { dir: '/home/user/.codex', logged_in: false, usage: null })],
      defaultAccountId: 'codex',
      degraded: true,
      relationships: {},
      resolvedBases: [],
    },
    agy: {
      accounts: [account('agy', { dir: '/home/user/.gemini' })],
      defaultAccountId: null,
      degraded: false,
      relationships: {},
      resolvedBases: [],
    },
    grok: {
      accounts: [account('grok', { dir: '/home/user/.grok', usage: null })],
      defaultAccountId: null,
      degraded: false,
      relationships: {},
      resolvedBases: [],
    },
  }
}

describe('AccountsHome', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders registry-order sections, full meters, freshness, and degraded state', () => {
    render(AccountsHome, { props: { states: states(), projects: [] } })

    expect(screen.getAllByTestId(/accounts-tool-/).map((section) => section.dataset.tool)).toEqual([
      'claude',
      'codex',
      'agy',
      'grok',
    ])
    expect(screen.getByTestId('accounts-freshness')).toHaveTextContent('Usage as of')
    expect(screen.getByTestId('accounts-degraded-banner')).toHaveTextContent(
      'Detection degraded — showing last-known accounts'
    )
    expect(screen.getByTestId('account-row-personal')).toContainElement(
      within(screen.getByTestId('account-row-personal')).getByTestId('usage-meter')
    )
    expect(screen.queryByTestId('add-account-agy')).not.toBeInTheDocument()
  })

  it('auto-expands unhealthy rows and exposes only home management actions', async () => {
    const onOpenProject = vi.fn()
    const onSignIn = vi.fn()
    render(AccountsHome, {
      props: { states: states(), projects: [], onOpenProject, onSignIn },
    })

    const work = screen.getByTestId('account-row-work')
    expect(within(work).getByTestId('account-row-details')).toBeInTheDocument()
    expect(within(work).getByText('taurhaus')).toBeInTheDocument()
    expect(within(work).getByText('wave-a')).toBeInTheDocument()

    await fireEvent.click(within(work).getByLabelText('Remove taurhaus pin'))
    expect(rememberChoice).toHaveBeenCalledWith('p1', 'claude', null)

    await fireEvent.click(within(work).getByText('Set as global default'))
    expect(setGlobalDefault).toHaveBeenCalledWith('claude', 'work')
    await fireEvent.click(within(work).getByText('Sign in…'))
    expect(onSignIn).toHaveBeenCalledWith('claude', expect.objectContaining({ id: 'work' }))
    await fireEvent.click(within(work).getByText('Reveal directory'))
    expect(revealDirectory).toHaveBeenCalledWith('/home/user/.claude-work')
  })

  // Regression: 971d964 sent a team link through the same callback as a pinned
  // project, so nothing downstream could tell one from the other and a team
  // link landed on whichever tab happened to be open instead of its mesh.
  it('opens a team link through its own callback', async () => {
    const onOpenProject = vi.fn()
    const onOpenTeam = vi.fn()
    render(AccountsHome, {
      props: { states: states(), projects: [], onOpenProject, onOpenTeam },
    })

    const work = screen.getByTestId('account-row-work')
    await fireEvent.click(within(work).getByText('wave-a'))

    expect(onOpenTeam).toHaveBeenCalledWith(
      expect.objectContaining({ name: 'wave-a', projectId: 'p1' })
    )
    expect(onOpenProject).not.toHaveBeenCalled()

    await fireEvent.click(within(work).getByText('taurhaus'))
    expect(onOpenProject).toHaveBeenCalledWith(expect.objectContaining({ id: 'p1' }))
  })

  // Regression: faffe345 treated an already-reset 100% snapshot as unhealthy,
  // auto-expanding a row whose meter correctly had no live exhausted window.
  it('keeps a just-reset account healthy and collapsed', () => {
    const accountStates = states()
    accountStates.claude.accounts = accountStates.claude.accounts.map((entry) =>
      entry.id === 'work'
        ? {
            ...entry,
            usage: {
              ...usage(100),
              windows: [
                {
                  ...usage(100).windows[0],
                  resets_at: Math.floor(now / 1000) - 60,
                },
              ],
            },
          }
        : entry
    )

    render(AccountsHome, { props: { states: accountStates, projects: [] } })

    const work = screen.getByTestId('account-row-work')
    expect(within(work).queryByTestId('account-row-details')).not.toBeInTheDocument()
    expect(within(work).getByRole('button', { name: 'Expand work' })).toBeInTheDocument()
    expect(work.querySelector('.bg-emerald-500')).toBeInTheDocument()
    expect(work.querySelector('.bg-rose-500')).not.toBeInTheDocument()
  })

  // Regression: 186f19a2 narrowed the row dot and the auto-expand to
  // percentage and exhaustion, so a provider severity that disagreed with the
  // percentage painted the meter bar and lit the ambient badge while the row
  // stayed emerald and collapsed.
  it('reads provider severity for the row health dot and auto-expansion', () => {
    const accountStates = states()
    const window = (used, severity) => ({
      ...usage(used),
      windows: [{ ...usage(used).windows[0], used_percentage: used, severity }],
    })
    accountStates.claude.accounts = [
      account('personal', { usage: window(50, 'warning') }),
      account('work', { usage: window(95, 'critical') }),
    ]

    render(AccountsHome, { props: { states: accountStates, projects: [] } })

    const personal = screen.getByTestId('account-row-personal')
    expect(within(personal).getByTestId('account-health-dot')).toHaveClass('bg-amber-500')
    expect(within(personal).queryByTestId('account-row-details')).not.toBeInTheDocument()

    const work = screen.getByTestId('account-row-work')
    expect(within(work).getByTestId('account-health-dot')).toHaveClass('bg-rose-500')
    expect(within(work).getByTestId('account-row-details')).toBeInTheDocument()
  })

  // Regression: faffe345 auto-expanded unhealthy rows from the same reactive
  // set the disclosure changed, so collapsing one immediately expanded it again.
  it('lets the user collapse an unhealthy row after its initial auto-expand', async () => {
    render(AccountsHome, { props: { states: states(), projects: [] } })

    const work = screen.getByTestId('account-row-work')
    expect(within(work).getByTestId('account-row-details')).toBeInTheDocument()

    await fireEvent.click(within(work).getByRole('button', { name: 'Collapse work' }))

    expect(within(work).queryByTestId('account-row-details')).not.toBeInTheDocument()
  })

  // Regression: 31521eb remembered every account ever auto-expanded, so a
  // recovered row would not expand when a later unhealthy episode began.
  it('auto-expands an account again after it recovers and becomes unhealthy later', async () => {
    const { rerender } = render(AccountsHome, { props: { states: states(), projects: [] } })

    let work = screen.getByTestId('account-row-work')
    await fireEvent.click(within(work).getByRole('button', { name: 'Collapse work' }))

    const healthy = states()
    healthy.claude.accounts = healthy.claude.accounts.map((entry) =>
      entry.id === 'work' ? { ...entry, usage: usage(20) } : entry
    )
    await rerender({ states: healthy, projects: [] })
    work = screen.getByTestId('account-row-work')
    expect(within(work).queryByTestId('account-row-details')).not.toBeInTheDocument()

    await rerender({ states: states(), projects: [] })
    work = screen.getByTestId('account-row-work')
    expect(within(work).getByTestId('account-row-details')).toBeInTheDocument()
  })

  it('explains base-command selectors and converts affected projects into pins', async () => {
    const accountStates = states()
    accountStates.claude.defaultAccountId = null
    const projects = [
      { id: 'p-free', name: 'free', accountMemory: {} },
      {
        id: 'p-settled',
        name: 'settled',
        accountMemory: { claude: { accountId: 'personal', origin: 'last_used' } },
      },
    ]
    render(AccountsHome, { props: { states: accountStates, projects } })

    expect(screen.getByTestId('account-alias-claude')).toHaveTextContent('claude2')
    await fireEvent.click(screen.getByText('Convert to pins'))

    expect(rememberChoice).toHaveBeenCalledWith('p-free', 'claude', 'work')
    expect(rememberChoice).not.toHaveBeenCalledWith('p-settled', 'claude', 'work')
  })

  // Regression: faffe345 compared the reported selector value verbatim with
  // absolute account directories, so a `~/`-spelled alias lost its conversion strip.
  it('explains a tilde-spelled base-command selector', () => {
    const accountStates = states()
    accountStates.claude.defaultAccountId = null
    accountStates.claude.resolvedBases[0].selectorValue = '~/.claude-work'

    render(AccountsHome, {
      props: {
        states: accountStates,
        projects: [{ id: 'p-free', name: 'free', accountMemory: {} }],
      },
    })

    expect(screen.getByTestId('account-alias-claude')).toHaveTextContent('claude2')
    expect(screen.getByText('Convert to pins')).toBeInTheDocument()
  })

  // Regression: 186f19a2 rendered the FR7 strip only when the backend reported
  // an alias expansion, so a base command that spells the selector out in
  // Settings got no explainer and no Convert to pins.
  it('explains a literally spelled base-command selector and converts affected projects', async () => {
    const accountStates = states()
    accountStates.claude.defaultAccountId = null
    accountStates.claude.resolvedBases = [
      {
        command: 'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude',
        selectorValue: '/home/user/.claude-work',
        expansions: [],
      },
    ]

    render(AccountsHome, {
      props: {
        states: accountStates,
        projects: [{ id: 'p-free', name: 'free', accountMemory: {} }],
      },
    })

    expect(screen.getByTestId('account-alias-claude')).toHaveTextContent(
      'CLAUDE_CONFIG_DIR=/home/user/.claude-work claude'
    )
    await fireEvent.click(screen.getByText('Convert to pins'))
    expect(rememberChoice).toHaveBeenCalledWith('p-free', 'claude', 'work')
  })

  // Regression: 186f19a2 dropped the opaque-head case the Settings authority
  // warns about first, so a launch command taurhaus cannot see through was
  // presented as if nothing decided the account.
  it('warns about an opaque base-command head instead of offering pins', () => {
    const accountStates = states()
    accountStates.claude.defaultAccountId = null
    accountStates.claude.resolvedBases = [
      {
        command: 'claude-wrapper',
        selectorValue: '/home/user/.claude-work',
        expansions: [],
        opaqueHead: 'claude-wrapper',
      },
    ]

    render(AccountsHome, {
      props: {
        states: accountStates,
        projects: [{ id: 'p-free', name: 'free', accountMemory: {} }],
      },
    })

    expect(screen.getByTestId('account-base-opaque-claude')).toHaveTextContent('claude-wrapper')
    expect(screen.queryByTestId('account-alias-claude')).not.toBeInTheDocument()
    expect(screen.queryByText('Convert to pins')).not.toBeInTheDocument()
  })

  // Regression: faffe345 offered to pin every memory-free project to the base
  // command account even when the configured global default already outranked it.
  it('does not offer alias conversion when a global default settles the projects', () => {
    render(AccountsHome, {
      props: {
        states: states(),
        projects: [{ id: 'p-free', name: 'free', accountMemory: {} }],
      },
    })

    expect(screen.queryByTestId('account-alias-claude')).not.toBeInTheDocument()
    expect(rememberChoice).not.toHaveBeenCalled()
  })

  // Regression: 971d964 left opener rejections unhandled, so Reveal directory
  // failed without telling the user what happened.
  it('shows a row-level error when revealing the account directory fails', async () => {
    revealDirectory.mockRejectedValueOnce(new Error('Explorer could not reveal this directory'))
    render(AccountsHome, { props: { states: states(), projects: [] } })

    const work = screen.getByTestId('account-row-work')
    await fireEvent.click(within(work).getByText('Reveal directory'))

    expect(await within(work).findByRole('status')).toHaveTextContent(
      'Explorer could not reveal this directory'
    )
  })

  // Regression: faffe345 exposed Sign in on every row even when the registry
  // declares no selector or login command, guaranteeing a dead-end action.
  it('hides sign-in actions for tools without a registry login command', async () => {
    render(AccountsHome, { props: { states: states(), projects: [] } })

    const agy = screen.getByTestId('account-row-agy')
    await fireEvent.click(within(agy).getByRole('button', { name: 'Expand agy' }))

    expect(within(agy).queryByText('Sign in…')).not.toBeInTheDocument()
  })

  // Regression: faffe345 refreshed usage for every registry tool, including
  // Grok even though its descriptor declares no usage provider.
  it('refreshes accounts for every tool but usage only for supported tools', async () => {
    render(AccountsHome, { props: { states: states(), projects: [] } })

    await fireEvent.click(screen.getByRole('button', { name: 'Refresh account usage' }))

    expect(refreshAccounts).toHaveBeenCalledWith('claude', { force: true })
    expect(refreshAccounts).toHaveBeenCalledWith('grok', { force: true })
    expect(refreshUsage).toHaveBeenCalledWith('claude')
    expect(refreshUsage).not.toHaveBeenCalledWith('grok')
  })
})
