import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import AccountChip from './AccountChip.svelte'

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

describe('AccountChip', () => {
  it('shows the generic resolution origin hint', () => {
    render(AccountChip, {
      props: {
        tool: 'claude',
        accounts: ACCOUNTS,
        selectedAccountId: 'account-2',
        origin: 'last_used',
        onSelect: vi.fn(),
      },
    })

    expect(screen.getByTestId('account-chip')).toHaveTextContent('last used')
  })

  it('stays hidden when only one account exists', () => {
    render(AccountChip, {
      props: { accounts: [ACCOUNTS[0]], selectedAccountId: null, onSelect: vi.fn() },
    })

    expect(screen.queryByTestId('account-chip')).not.toBeInTheDocument()
  })

  it('shows the default account when the project stored no choice', () => {
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: null, onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    expect(chip).toHaveTextContent('Who')
    expect(chip).toHaveAttribute('title', expect.stringContaining('stierms@gmail.com'))
  })

  it('shows the project account and changes it through the menu', async () => {
    const onSelect = vi.fn()
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-2', onSelect },
    })

    expect(screen.getByTestId('account-chip')).toHaveTextContent('Matthias')

    await fireEvent.click(screen.getByTestId('account-chip'))
    await fireEvent.click(screen.getByTestId('account-menu-item-account-1'))

    expect(onSelect).toHaveBeenCalledWith('account-1')
  })

  // Regression: c982822 read the inherited account off `is_default`, which is
  // the physical `~/.claude` dir, so a project inheriting a global default
  // configured in Settings advertised the wrong subscription.
  it('shows the configured global default for an inheriting project', () => {
    render(AccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: null,
        defaultAccountId: 'account-2',
        onSelect: vi.fn(),
      },
    })

    const chip = screen.getByTestId('account-chip')
    expect(chip).toHaveTextContent('Matthias')
    expect(chip).toHaveAttribute('title', expect.stringContaining('m.stier@giesi.com'))
  })

  it('falls back to the default config dir when the configured default is gone', () => {
    render(AccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: null,
        defaultAccountId: 'deleted-account',
        onSelect: vi.fn(),
      },
    })

    expect(screen.getByTestId('account-chip')).toHaveTextContent('Who')
  })

  it('offers clearing the project choice back to the default', async () => {
    const onSelect = vi.fn()
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-2', onSelect },
    })

    await fireEvent.click(screen.getByTestId('account-chip'))
    await fireEvent.click(screen.getByTestId('account-menu-clear'))

    expect(onSelect).toHaveBeenCalledWith(null)
  })

  // Regression: 518aace read a daemon failure as an empty account list, so the
  // chip vanished mid-session and the project's subscription went unnamed.
  it('stays visible and says the list is stale when detection is degraded', async () => {
    render(AccountChip, {
      props: {
        accounts: ACCOUNTS,
        selectedAccountId: 'account-2',
        degraded: true,
        onSelect: vi.fn(),
      },
    })

    const chip = screen.getByTestId('account-chip')
    expect(chip).toHaveTextContent('Matthias')

    await fireEvent.click(chip)
    expect(screen.getByTestId('accounts-degraded')).toHaveTextContent(
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
    render(AccountChip, {
      props: { accounts, selectedAccountId: 'account-2', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    expect(chip).toHaveTextContent('5h 81%')

    await fireEvent.click(chip)
    expect(screen.getByTestId('account-menu-item-account-1')).toHaveTextContent('5h 26%')
    expect(screen.getByTestId('account-menu-item-account-2')).toHaveTextContent('7d 44%')
  })

  // Regression: 79be608 only ever read usage during account *detection*, which
  // OverviewTab ran once on mount. A project mounted before its session's first
  // status-line payload kept an empty meter for as long as it stayed open, and
  // opening the menu to compare subscriptions showed numbers from mount time.
  it('asks for fresh usage when the menu is opened', async () => {
    const onRequestUsage = vi.fn()
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onRequestUsage, onSelect: vi.fn() },
    })

    expect(onRequestUsage).not.toHaveBeenCalled()

    await fireEvent.click(screen.getByTestId('account-chip'))

    expect(onRequestUsage).toHaveBeenCalledTimes(1)
  })

  it('says nothing about usage for an account no status line has reported', async () => {
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    expect(screen.getByTestId('account-chip')).not.toHaveTextContent('%')

    await fireEvent.click(screen.getByTestId('account-chip'))
    expect(screen.queryByTestId('usage-meter')).not.toBeInTheDocument()
  })

  // Regression: c982822 positioned this menu `absolute` inside the Overview
  // header, so it was laid out against whatever ancestor happened to be
  // positioned and clipped by the `overflow-hidden` main panel. It is a popup:
  // it belongs to the viewport, measured and clamped like `ContextMenu`.
  it('anchors the menu to the viewport instead of an ancestor', async () => {
    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    vi.spyOn(chip, 'getBoundingClientRect').mockReturnValue({
      left: 300, top: 60, right: 420, bottom: 82, width: 120, height: 22, x: 300, y: 60,
      toJSON() {},
    })

    await fireEvent.click(chip)

    const menu = screen.getByTestId('account-menu')
    expect(menu.className).toContain('fixed')
    expect(menu.className).not.toContain('absolute')
    expect(menu.style.top).toBe('86px')
  })

  it('flips the menu above the chip when the viewport has no room below', async () => {
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 300 })

    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    vi.spyOn(chip, 'getBoundingClientRect').mockReturnValue({
      left: 300, top: 250, right: 420, bottom: 272, width: 120, height: 22, x: 300, y: 250,
      toJSON() {},
    })

    await fireEvent.click(chip)

    const menu = screen.getByTestId('account-menu')
    vi.spyOn(menu, 'getBoundingClientRect').mockReturnValue({
      left: 0, top: 0, right: 224, bottom: 200, width: 224, height: 200, x: 0, y: 0,
      toJSON() {},
    })
    // Re-measure with the menu's real height in hand, the way a resize does.
    await fireEvent(window, new Event('resize'))

    expect(Number.parseInt(menu.style.top, 10)).toBeLessThan(250)

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
  })

  // Regression: 74c7761 re-clamped this menu on a scroll or a window resize and
  // on nothing else. Opening it asks for usage, and the meters that answer make
  // both the chip and the menu bigger — so a menu opened near the bottom edge
  // kept the top it was given while it was empty and ran off the screen.
  it('re-clamps when the usage it asked for arrives', async () => {
    const observers = []
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback) {
        this.callback = callback
        this.targets = []
        observers.push(this)
      }
      observe(target) { this.targets.push(target) }
      disconnect() {}
    })
    const previousHeight = window.innerHeight
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 400 })

    const { rerender } = render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    // The chip grows a meter of its own, so its right edge moves with the menu.
    let chipRight = 420
    vi.spyOn(chip, 'getBoundingClientRect').mockImplementation(() => ({
      left: 300, top: 300, right: chipRight, bottom: 322,
      width: chipRight - 300, height: 22, x: 300, y: 300, toJSON() {},
    }))

    await fireEvent.click(chip)

    const menu = screen.getByTestId('account-menu')
    let menuHeight = 60
    vi.spyOn(menu, 'getBoundingClientRect').mockImplementation(() => ({
      left: 0, top: 0, right: 224, bottom: menuHeight,
      width: 224, height: menuHeight, x: 0, y: 0, toJSON() {},
    }))
    await waitFor(() => expect(menu.style.top).toBe('326px'))
    expect(menu.style.left).toBe('196px')

    // The numbers landed: two meters in the menu, one on the chip.
    await rerender({
      accounts: ACCOUNTS.map((account) => ({ ...account, usage: usageAt(40, 30, 1) })),
    })
    menuHeight = 200
    chipRight = 500
    observers.at(-1).callback([])

    // No room below any more: above the chip, and right-aligned to its new edge.
    await waitFor(() => expect(menu.style.top).toBe('96px'))
    expect(menu.style.left).toBe('276px')
    expect(observers.at(-1).targets).toEqual(expect.arrayContaining([chip, menu]))

    Object.defineProperty(window, 'innerHeight', { configurable: true, value: previousHeight })
    vi.unstubAllGlobals()
  })

  it('clamps the menu inside the right edge of a narrow viewport', async () => {
    const previousWidth = window.innerWidth
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 400 })

    render(AccountChip, {
      props: { accounts: ACCOUNTS, selectedAccountId: 'account-1', onSelect: vi.fn() },
    })

    const chip = screen.getByTestId('account-chip')
    vi.spyOn(chip, 'getBoundingClientRect').mockReturnValue({
      left: 320, top: 60, right: 390, bottom: 82, width: 70, height: 22, x: 320, y: 60,
      toJSON() {},
    })

    await fireEvent.click(chip)

    const menu = screen.getByTestId('account-menu')
    vi.spyOn(menu, 'getBoundingClientRect').mockReturnValue({
      left: 0, top: 0, right: 224, bottom: 200, width: 224, height: 200, x: 0, y: 0,
      toJSON() {},
    })
    await fireEvent(window, new Event('resize'))

    const left = Number.parseInt(menu.style.left, 10)
    expect(left).toBeGreaterThanOrEqual(8)
    expect(left + 224).toBeLessThanOrEqual(400 - 8)

    Object.defineProperty(window, 'innerWidth', { configurable: true, value: previousWidth })
  })
})
