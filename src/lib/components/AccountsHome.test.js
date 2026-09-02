import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

vi.mock('../accounts.svelte.js', () => ({
  accountState: vi.fn(() => ({ accounts: [], relationships: {}, resolvedBases: [] })),
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

  it('explains base-command selectors and converts affected projects into pins', async () => {
    const projects = [
      { id: 'p-free', name: 'free', accountMemory: {} },
      {
        id: 'p-settled',
        name: 'settled',
        accountMemory: { claude: { accountId: 'personal', origin: 'last_used' } },
      },
    ]
    render(AccountsHome, { props: { states: states(), projects } })

    expect(screen.getByTestId('account-alias-claude')).toHaveTextContent('claude2')
    await fireEvent.click(screen.getByText('Convert to pins'))

    expect(rememberChoice).toHaveBeenCalledWith('p-free', 'claude', 'work')
    expect(rememberChoice).not.toHaveBeenCalledWith('p-settled', 'claude', 'work')
  })

  it('refreshes every registry tool from the header', async () => {
    render(AccountsHome, { props: { states: states(), projects: [] } })

    await fireEvent.click(screen.getByRole('button', { name: 'Refresh account usage' }))

    expect(refreshAccounts).toHaveBeenCalledWith('claude', { force: true })
    expect(refreshAccounts).toHaveBeenCalledWith('grok', { force: true })
  })
})
