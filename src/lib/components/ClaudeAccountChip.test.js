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
})
