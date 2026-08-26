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

  it('offers clearing the project choice back to the default', async () => {
    const onSelect = vi.fn()
    render(ClaudeAccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-2', onSelect },
    })

    await fireEvent.click(screen.getByTestId('claude-account-chip'))
    await fireEvent.click(screen.getByTestId('claude-account-menu-clear'))

    expect(onSelect).toHaveBeenCalledWith(null)
  })
})
