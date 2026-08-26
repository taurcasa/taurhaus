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

  // Regression: c982822 took Enter's answer from `is_default` — the physical
  // `~/.claude` dir — so the keystroke pinned the project to an account the
  // user had already replaced as the global default in Settings.
  it('Enter takes the configured global default and the badge marks it', async () => {
    const { onConfirm } = renderChooser({ defaultAccountId: 'account-2' })

    expect(screen.getByTestId('claude-account-option-account-2')).toContainElement(
      screen.getByTestId('claude-account-default-badge')
    )

    await fireEvent.keyDown(screen.getByTestId('claude-account-chooser'), { key: 'Enter' })

    expect(onConfirm).toHaveBeenCalledWith('account-2', true)
  })

  it('Enter ignores a configured default that cannot run', async () => {
    const { onConfirm } = renderChooser({ defaultAccountId: 'account-3' })

    await fireEvent.keyDown(screen.getByTestId('claude-account-chooser'), { key: 'Enter' })

    expect(onConfirm).toHaveBeenCalledWith('account-1', true)
  })

  // Regression: 518aace read a daemon failure as an empty account list, so the
  // chooser stopped appearing altogether. A stale list is still the answer to
  // "which subscriptions do I have" — it just needs to say that it is stale.
  it('keeps offering the last known accounts when detection is degraded', () => {
    renderChooser({ degraded: true })

    expect(screen.getByTestId('claude-account-option-account-1')).toBeInTheDocument()
    expect(screen.getByTestId('claude-accounts-degraded')).toHaveTextContent(
      'Accounts unavailable (daemon offline) — using last known'
    )
  })

  it('says nothing about the daemon while detection works', () => {
    renderChooser()

    expect(screen.queryByTestId('claude-accounts-degraded')).not.toBeInTheDocument()
  })
})
