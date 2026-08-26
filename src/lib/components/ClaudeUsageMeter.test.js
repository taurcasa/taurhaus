import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import '@testing-library/jest-dom/vitest'

import ClaudeUsageMeter from './ClaudeUsageMeter.svelte'

const NOW = new Date('2026-08-27T12:00:00Z')

function usage({ observedMinutesAgo = 1, fiveHour = 26, sevenDay = 17 } = {}) {
  return {
    five_hour:
      fiveHour == null
        ? null
        : { used_percentage: fiveHour, resets_at: Math.floor(NOW.getTime() / 1000) + 3 * 3600 },
    seven_day:
      sevenDay == null
        ? null
        : { used_percentage: sevenDay, resets_at: Math.floor(NOW.getTime() / 1000) + 40 * 3600 },
    observed_at: new Date(NOW.getTime() - observedMinutesAgo * 60_000).toISOString(),
  }
}

describe('ClaudeUsageMeter', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(NOW)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  // Regression: d6839a3 showed subscriptions with no usage at all. An account
  // that has never run a session under taurhaus has no record, and rendering
  // that as 0 % would send the user to the subscription with the least
  // headroom.
  it('renders nothing at all when no status line has reported', () => {
    const { container } = render(ClaudeUsageMeter, { props: { usage: null } })

    expect(container.textContent.trim()).toBe('')
    expect(screen.queryByTestId('claude-usage-meter')).not.toBeInTheDocument()
  })

  it('shows both windows and when the nearer one resets', () => {
    render(ClaudeUsageMeter, { props: { usage: usage() } })

    const meter = screen.getByTestId('claude-usage-meter')
    expect(meter).toHaveTextContent('5h 26%')
    expect(meter).toHaveTextContent('7d 17%')
    expect(screen.getByTestId('claude-usage-reset')).toHaveTextContent('resets in 3h')
    expect(screen.queryByTestId('claude-usage-stale')).not.toBeInTheDocument()
    expect(screen.getByTestId('claude-usage-bar-five-hour')).toHaveStyle({ width: '26%' })
  })

  // Regression: d6839a3 had no usage. Usage only flows while a session of that
  // account is running, so a record is routinely hours old — presenting a stale
  // percentage as the current one is the failure mode this whole feature has.
  it('says when the numbers were last seen instead of when they reset', () => {
    render(ClaudeUsageMeter, { props: { usage: usage({ observedMinutesAgo: 200 }) } })

    expect(screen.getByTestId('claude-usage-stale')).toHaveTextContent('last seen 3h ago')
    expect(screen.queryByTestId('claude-usage-reset')).not.toBeInTheDocument()
    expect(screen.getByTestId('claude-usage-meter')).toHaveTextContent('5h 26%')
  })

  it('shows the window it has when the payload carried only one', () => {
    render(ClaudeUsageMeter, { props: { usage: usage({ sevenDay: null }) } })

    const meter = screen.getByTestId('claude-usage-meter')
    expect(meter).toHaveTextContent('5h 26%')
    expect(meter).not.toHaveTextContent('7d')
  })

  it('collapses to one line with no bars where the chip has no room', () => {
    render(ClaudeUsageMeter, { props: { usage: usage(), compact: true } })

    expect(screen.getByTestId('claude-usage-meter')).toHaveTextContent('5h 26% · 7d 17%')
    expect(screen.queryByTestId('claude-usage-bar-five-hour')).not.toBeInTheDocument()
    expect(screen.queryByTestId('claude-usage-reset')).not.toBeInTheDocument()
  })

  it('never draws a bar past its track', () => {
    render(ClaudeUsageMeter, { props: { usage: usage({ fiveHour: 143 }) } })

    expect(screen.getByTestId('claude-usage-bar-five-hour')).toHaveStyle({ width: '100%' })
    expect(screen.getByTestId('claude-usage-meter')).toHaveTextContent('5h 143%')
  })
})
