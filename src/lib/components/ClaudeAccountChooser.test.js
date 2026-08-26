import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ClaudeAccountChooser from './ClaudeAccountChooser.svelte'

const ACCOUNTS = [
  {
    id: 'account-1',
    email: 'stierms@gmail.com',
    display_name: 'Who',
    organization: "stierms@gmail.com's Organization",
    seat_tier: 'claude_max',
    logged_in: true,
    is_default: true,
  },
  {
    id: 'account-2',
    email: 'm.stier@giesi.com',
    display_name: 'Matthias',
    organization: "m.stier@giesi.com's Organization",
    seat_tier: 'claude_max',
    logged_in: true,
    is_default: false,
  },
  {
    id: 'account-3',
    email: 'logged-out@example.com',
    display_name: null,
    organization: null,
    seat_tier: null,
    logged_in: false,
    is_default: false,
  },
]

function renderChooser(overrides = {}) {
  const onConfirm = vi.fn()
  const onCancel = vi.fn()
  render(ClaudeAccountChooser, {
    props: {
      accounts: ACCOUNTS,
      projectName: 'taurhaus',
      onConfirm,
      onCancel,
      ...overrides,
    },
  })
  return { onConfirm, onCancel }
}

describe('ClaudeAccountChooser', () => {
  it('lists every account and marks the default', () => {
    renderChooser()

    expect(screen.getByTestId('claude-account-option-account-1')).toHaveTextContent('Who')
    expect(screen.getByTestId('claude-account-option-account-1')).toHaveTextContent(
      'stierms@gmail.com'
    )
    expect(screen.getByTestId('claude-account-default-badge')).toBeInTheDocument()
  })

  it('disables an account that is not logged in', () => {
    renderChooser()

    expect(screen.getByTestId('claude-account-option-account-3')).toBeDisabled()
    expect(screen.getByTestId('claude-account-option-account-3')).toHaveTextContent('Not logged in')
  })

  it('remembers the choice by default and reports both back', async () => {
    const { onConfirm } = renderChooser()

    await fireEvent.click(screen.getByTestId('claude-account-option-account-2'))

    expect(onConfirm).toHaveBeenCalledWith('account-2', true)
  })

  it('passes remember=false when the checkbox is cleared', async () => {
    const { onConfirm } = renderChooser()

    await fireEvent.click(screen.getByTestId('claude-account-remember'))
    await fireEvent.click(screen.getByTestId('claude-account-option-account-2'))

    expect(onConfirm).toHaveBeenCalledWith('account-2', false)
  })

  it('Enter picks the default account and Escape cancels', async () => {
    const { onConfirm, onCancel } = renderChooser()
    const panel = screen.getByTestId('claude-account-chooser')

    await fireEvent.keyDown(panel, { key: 'Enter' })
    expect(onConfirm).toHaveBeenCalledWith('account-1', true)

    await fireEvent.keyDown(panel, { key: 'Escape' })
    expect(onCancel).toHaveBeenCalled()
  })
})
