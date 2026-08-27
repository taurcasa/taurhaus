import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ClaudeAccountChooser from './ClaudeAccountChooser.svelte'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

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

function usageAt(fiveHour, sevenDay, minutesAgo) {
  const now = Date.now()
  return {
    five_hour: { used_percentage: fiveHour, resets_at: Math.floor(now / 1000) + 3600 },
    seven_day: { used_percentage: sevenDay, resets_at: Math.floor(now / 1000) + 90000 },
    observed_at: new Date(now - minutesAgo * 60_000).toISOString(),
  }
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
  // Regression: d6839a3 asked which subscription to run on without saying what
  // was left of either one — the single moment the answer actually matters.
  it('shows each account usage so the user can pick the one with headroom', () => {
    render(ClaudeAccountChooser, {
      props: {
        accounts: [
          { ...ACCOUNTS[0], usage: usageAt(91, 62, 1) },
          { ...ACCOUNTS[1], usage: usageAt(12, 8, 1) },
          ACCOUNTS[2],
        ],
        projectName: 'taurhaus',
        onConfirm: vi.fn(),
        onCancel: vi.fn(),
      },
    })

    expect(screen.getByTestId('claude-account-option-account-1')).toHaveTextContent('5h 91%')
    expect(screen.getByTestId('claude-account-option-account-2')).toHaveTextContent('5h 12%')
    // The logged-out account has no record, and an empty meter is the answer.
    expect(screen.getByTestId('claude-account-option-account-3')).not.toHaveTextContent('%')
  })

  // Regression: c982822 mounted this chooser as a bare `fixed inset-0` div
  // directly inside `.shell-frame`, where `.shell-frame > :not([data-shell-
  // overlay])` sets `position: relative` on every child. The dialog stopped
  // being an overlay and became the last item of the frame's column: the user
  // saw it at the bottom of the window, half cut off. The other two overlays
  // in the app carry the opt-out attribute; this one did not.
  it('mounts as a viewport overlay the shell frame does not reposition', () => {
    renderChooser()

    const overlay = screen.getByTestId('claude-account-chooser-overlay')
    expect(overlay).toHaveAttribute('data-shell-overlay')
    expect(overlay.className).toContain('fixed')
    expect(overlay.className).toContain('inset-0')
    expect(overlay).toContainElement(screen.getByTestId('claude-account-chooser'))

    expect(appCss).toContain('.shell-frame > :not([data-shell-overlay])')
  })

  // Regression: c982822 pinned the dialog to `pt-24` with no height cap, so a
  // three-account chooser on a short window ran off the bottom edge with no
  // way to reach the accounts below the fold.
  it('centres the dialog and scrolls it instead of overflowing the viewport', () => {
    renderChooser()

    const overlay = screen.getByTestId('claude-account-chooser-overlay')
    expect(overlay.className).toContain('items-center')
    expect(overlay.className).not.toContain('pt-24')

    const dialog = screen.getByTestId('claude-account-chooser')
    expect(dialog.className).toContain('max-h-[calc(100vh-4rem)]')
    expect(dialog.className).toContain('overflow-y-auto')
  })
})
