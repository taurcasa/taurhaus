import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, within } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import AccountPicker from './AccountPicker.svelte'

const ACCOUNTS = [
  {
    id: 'personal',
    label: 'personal@example.com',
    display_name: 'Personal',
    logged_in: true,
    is_default: true,
  },
  {
    id: 'work',
    label: 'work@example.com',
    display_name: 'Work',
    logged_in: true,
    is_default: false,
  },
]

describe('AccountPicker', () => {
  it('keeps choice rows and usage in one skin-independent core', async () => {
    const onConfirm = vi.fn()
    render(AccountPicker, {
      props: {
        tool: 'claude',
        accounts: ACCOUNTS,
        skin: 'popover',
        onConfirm,
      },
    })

    expect(screen.getByTestId('account-picker')).toHaveAttribute('data-skin', 'popover')
    await fireEvent.click(screen.getByTestId('account-option-work'))
    expect(onConfirm).toHaveBeenCalledWith('work', true)
  })

  it('states the two Wave A scopes without inventing another scope', async () => {
    render(AccountPicker, { props: { tool: 'claude', accounts: ACCOUNTS } })

    expect(screen.getByText('Use for this project')).toBeInTheDocument()
    expect(screen.getByText('Otherwise, this launch only.')).toBeInTheDocument()

    await fireEvent.click(screen.getByTestId('account-remember'))
    expect(screen.getByText('This launch only.')).toBeInTheDocument()
    expect(screen.queryByText(/member|team/i)).not.toBeInTheDocument()
  })

  it('has an austere footer with only add and manage actions', async () => {
    const onAddAccount = vi.fn()
    const onManageAccounts = vi.fn()
    render(AccountPicker, {
      props: { tool: 'claude', accounts: ACCOUNTS, onAddAccount, onManageAccounts },
    })

    const footer = screen.getByTestId('account-picker-footer')
    expect(within(footer).getAllByRole('button')).toHaveLength(2)
    expect(within(footer).getByText('Add account…')).toBeInTheDocument()
    expect(within(footer).getByText('Manage accounts →')).toBeInTheDocument()
    expect(footer).not.toHaveTextContent(/default|pin|reveal/i)

    await fireEvent.click(within(footer).getByText('Add account…'))
    await fireEvent.click(within(footer).getByText('Manage accounts →'))
    expect(onAddAccount).toHaveBeenCalledWith('claude')
    expect(onManageAccounts).toHaveBeenCalledWith('claude')
  })
})
