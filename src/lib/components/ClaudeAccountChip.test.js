import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ClaudeAccountChip from './ClaudeAccountChip.svelte'

const ACCOUNTS = [
  {
    id: 'account-1',
    email: 'stierms@gmail.com',
    display_name: 'Who',
    logged_in: true,
    is_default: true,
  },
  {
    id: 'account-2',
    email: 'm.stier@giesi.com',
    display_name: 'Matthias',
    logged_in: true,
    is_default: false,
  },
]

function usageAt(fiveHour, sevenDay, minutesAgo) {
  const now = Date.now()
  return {
    five_hour: { used_percentage: fiveHour, resets_at: Math.floor(now / 1000) + 3600 },
    seven_day: { used_percentage: sevenDay, resets_at: Math.floor(now / 1000) + 90000 },
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
  }
}

describe('ClaudeAccountChip', () => {
  it('stays hidden when only one account exists', () => {
    render(ClaudeAccountChip, {
      props: { accounts: [ACCOUNTS[0]], selectedAccountId: null, onSelect: vi.fn() },
    })

    expect(screen.queryByTestId('claude-account-chip')).not.toBeInTheDocument()
  })

  it('shows the default account when the project stored no choice', () => {
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: null, onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('claude-account-chip')
    expect(chip).toHaveTextContent('Who')
    expect(chip).toHaveAttribute('title', expect.stringContaining('stierms@gmail.com'))
  })

  it('shows the project account and changes it through the menu', async () => {
    const onSelect = vi.fn()
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-2', onSelect },
    })

    expect(screen.getByTestId('claude-account-chip')).toHaveTextContent('Matthias')

    await fireEvent.click(screen.getByTestId('claude-account-chip'))
    await fireEvent.click(screen.getByTestId('claude-account-menu-item-account-1'))

    expect(onSelect).toHaveBeenCalledWith('account-1')
  })

  // Regression: c982822 read the inherited account off `is_default`, which is
  // the physical `~/.claude` dir, so a project inheriting a global default
  // configured in Settings advertised the wrong subscription.
  it('shows the configured global default for an inheriting project', () => {
    render(ClaudeAccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: null,
        defaultAccountId: 'account-2',
        onSelect: vi.fn(),
      },
    })

    const chip = screen.getByTestId('claude-account-chip')
    expect(chip).toHaveTextContent('Matthias')
    expect(chip).toHaveAttribute('title', expect.stringContaining('m.stier@giesi.com'))
  })

  it('falls back to the default config dir when the configured default is gone', () => {
    render(ClaudeAccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: null,
        defaultAccountId: 'deleted-account',
        onSelect: vi.fn(),
      },
    })

    expect(screen.getByTestId('claude-account-chip')).toHaveTextContent('Who')
  })

  it('offers clearing the project choice back to the default', async () => {
    const onSelect = vi.fn()
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-2', onSelect },
    })

    await fireEvent.click(screen.getByTestId('claude-account-chip'))
    await fireEvent.click(screen.getByTestId('claude-account-menu-clear'))

    expect(onSelect).toHaveBeenCalledWith(null)
  })

  // Regression: 518aace read a daemon failure as an empty account list, so the
  // chip vanished mid-session and the project's subscription went unnamed.
  it('stays visible and says the list is stale when detection is degraded', async () => {
    render(ClaudeAccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: 'account-2',
        degraded: true,
        onSelect: vi.fn(),
      },
    })

    const chip = screen.getByTestId('claude-account-chip')
    expect(chip).toHaveTextContent('Matthias')

    await fireEvent.click(chip)
    expect(screen.getByTestId('claude-accounts-degraded')).toHaveTextContent(
      'Accounts unavailable (daemon offline) — using last known'
    )
  })
  // Regression: d6839a3 named the subscription a project runs on but never
  // said what was left of it, so choosing between two Max accounts meant
  // guessing which one still had headroom.
  it('shows the selected account usage on the chip and every account usage in the menu', async () => {
    const accounts = [
      { ...ACCOUNTS[0], usage: usageAt(26, 17, 2) },
      { ...ACCOUNTS[1], usage: usageAt(81, 44, 2) },
    ]
    render(ClaudeAccountChip, {
      props: { accounts, selectedAccountId: 'account-2', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('claude-account-chip')
    expect(chip).toHaveTextContent('5h 81%')

    await fireEvent.click(chip)
    expect(screen.getByTestId('claude-account-menu-item-account-1')).toHaveTextContent('5h 26%')
    expect(screen.getByTestId('claude-account-menu-item-account-2')).toHaveTextContent('7d 44%')
  })

  // Regression: 79be608 only ever read usage during account *detection*, which
  // OverviewTab ran once on mount. A project mounted before its session's first
  // status-line payload kept an empty meter for as long as it stayed open, and
  // opening the menu to compare subscriptions showed numbers from mount time.
  it('asks for fresh usage when the menu is opened', async () => {
    const onRequestUsage = vi.fn()
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onRequestUsage, onSelect: vi.fn() },
    })

    expect(onRequestUsage).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('claude-account-chip'))

    expect(onRequestUsage).toHaveBeenCalledTimes(1)
  })

  it('says nothing about usage for an account no status line has reported', async () => {
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    expect(screen.getByTestId('claude-account-chip')).not.toHaveTextContent('%')

    await fireEvent.click(screen.getByTestId('claude-account-chip'))
    expect(screen.queryByTestId('claude-usage-meter')).not.toBeInTheDocument()
  })
})
