import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/svelte'
import { tick } from 'svelte'
import '@testing-library/jest-dom/vitest'

import UsageMeter from './UsageMeter.svelte'

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

describe('UsageMeter', () => {
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
    const { container } = render(UsageMeter, { props: { usage: null } })

    expect(container.textContent.trim()).toBe('')
    expect(screen.queryByTestId('usage-meter')).not.toBeInTheDocument()
  })

  it('shows both windows and when the nearer one resets', () => {
    render(UsageMeter, { props: { usage: usage() } })

    const meter = screen.getByTestId('usage-meter')
    expect(meter).toHaveTextContent('5h 26%')
    expect(meter).toHaveTextContent('7d 17%')
    expect(screen.getByTestId('usage-reset')).toHaveTextContent('resets in 3h')
    expect(screen.queryByTestId('usage-stale')).not.toBeInTheDocument()
    expect(screen.getByTestId('usage-bar-five-hour')).toHaveStyle({ width: '26%' })
  })

  // Regression: d6839a3 had no usage. Usage only flows while a session of that
  // account is running, so a record is routinely hours old — presenting a stale
  // percentage as the current one is the failure mode this whole feature has.
  it('says when the numbers were last seen instead of when they reset', () => {
    render(UsageMeter, { props: { usage: usage({ observedMinutesAgo: 200 }) } })

    expect(screen.getByTestId('usage-stale')).toHaveTextContent('last seen 3h ago')
    expect(screen.queryByTestId('usage-reset')).not.toBeInTheDocument()
    expect(screen.getByTestId('usage-meter')).toHaveTextContent('5h 26%')
  })

  it('renders an age-based dashed last-known state for provider windows', () => {
    const providerUsage = {
      status: 'ok',
      observed_at: new Date(NOW.getTime() - 16 * 60_000).toISOString(),
      windows: [
        {
          key: 'weekly',
          title: 'Week',
          used_percentage: 42,
          resets_at: Math.floor(NOW.getTime() / 1000) + 40 * 3600,
          severity: 'normal',
        },
      ],
    }

    render(UsageMeter, { props: { usage: providerUsage } })

    expect(screen.getByTestId('usage-meter')).toHaveAttribute('data-last-known', 'true')
    expect(screen.getByTestId('usage-last-known')).toHaveTextContent('Last known · 16m ago')
    expect(screen.getByTestId('usage-track-weekly')).toHaveClass('border-dashed')
    // Regression: 67fea5f hid the still-valid absolute reset time whenever the
    // observed percentage crossed the last-known age threshold.
    expect(screen.getByTestId('usage-reset')).toHaveTextContent('Resets')
  })

  it('shows the window it has when the payload carried only one', () => {
    render(UsageMeter, { props: { usage: usage({ sevenDay: null }) } })

    const meter = screen.getByTestId('usage-meter')
    expect(meter).toHaveTextContent('5h 26%')
    expect(meter).not.toHaveTextContent('7d')
  })

  it('collapses to one line with no bars where the chip has no room', () => {
    render(UsageMeter, { props: { usage: usage(), compact: true } })

    expect(screen.getByTestId('usage-meter')).toHaveTextContent('5h 26% · 7d 17%')
    expect(screen.queryByTestId('usage-bar-five-hour')).not.toBeInTheDocument()
    expect(screen.queryByTestId('usage-reset')).not.toBeInTheDocument()
  })

  it('keeps only provider-designated weekly buckets in compact mode', () => {
    // Regression: 5680a7a rendered every Codex window in the compact chip,
    // including 5h buckets whose dotted titles collided with its separators.
    const resets_at = Math.floor(NOW.getTime() / 1000) + 90_000
    render(UsageMeter, {
      props: {
        tool: 'codex',
        compact: true,
        usage: {
          observed_at: NOW.toISOString(),
          windows: [
            { key: 'codex.5h', title: '5h limit', used_percentage: 20, resets_at, compact: false },
            { key: 'codex.weekly', title: 'Weekly limit', used_percentage: 50, resets_at, compact: true },
            {
              key: 'codex_bengalfox.weekly',
              title: 'GPT-5.3-Codex-Spark · Weekly limit',
              used_percentage: 3,
              resets_at,
              compact: true,
            },
          ],
        },
      },
    })

    expect(screen.getByTestId('usage-meter')).toHaveTextContent(
      'Weekly limit 50% · GPT-5.3-Codex-Spark Weekly limit 3%'
    )
    expect(screen.getByTestId('usage-meter')).not.toHaveTextContent('5h limit')
  })

  it('falls back to every non-session window when a provider flags none compact', () => {
    // Regression: 2c49132 gated the compact meter on the `compact` flag as soon
    // as any window carried one, so a Codex snapshot whose windows are all
    // unflagged (a plan whose weekly bucket the provider never classified)
    // filtered down to nothing and the chip and settings meters rendered empty.
    const resets_at = Math.floor(NOW.getTime() / 1000) + 90_000
    render(UsageMeter, {
      props: {
        tool: 'codex',
        compact: true,
        usage: {
          observed_at: NOW.toISOString(),
          windows: [
            { key: 'codex.5h', title: '5h limit', used_percentage: 20, resets_at, compact: false },
            {
              key: 'codex.weekly',
              title: 'Weekly limit',
              used_percentage: 50,
              resets_at,
              compact: false,
            },
          ],
        },
      },
    })

    expect(screen.getByTestId('usage-meter')).toHaveTextContent('5h limit 20% · Weekly limit 50%')
  })

  it('shows the session window only when it is the sole window left', () => {
    // Regression: 2c49132 left a session-only snapshot with nothing to render in
    // compact surfaces, so an account mid-session showed no headroom at all.
    const resets_at = Math.floor(NOW.getTime() / 1000) + 3600
    render(UsageMeter, {
      props: {
        tool: 'claude',
        compact: true,
        usage: {
          observed_at: NOW.toISOString(),
          windows: [
            {
              key: 'session',
              title: 'Current session',
              used_percentage: 12,
              resets_at,
              compact: false,
            },
          ],
        },
      },
    })

    expect(screen.getByTestId('usage-meter')).toHaveTextContent('Current session 12%')
  })

  it('keeps a Claude snapshot compacted to its flagged weekly windows', () => {
    // Regression guard for 2c49132's fallback: providers that do flag windows
    // must still lose "Current session" from the chip.
    const resets_at = Math.floor(NOW.getTime() / 1000) + 90_000
    render(UsageMeter, {
      props: {
        tool: 'claude',
        compact: true,
        usage: {
          observed_at: NOW.toISOString(),
          windows: [
            {
              key: 'session',
              title: 'Current session',
              used_percentage: 12,
              resets_at,
              compact: false,
            },
            {
              key: 'weekly_all',
              title: 'Current week (all models)',
              used_percentage: 42,
              resets_at,
              compact: true,
            },
            {
              key: 'weekly_scoped:opus:1',
              title: 'Current week (Opus)',
              used_percentage: 61,
              resets_at,
              compact: true,
            },
          ],
        },
      },
    })

    expect(screen.getByTestId('usage-meter')).toHaveTextContent(
      'Current week (all models) 42% · Current week (Opus) 61%'
    )
    expect(screen.getByTestId('usage-meter')).not.toHaveTextContent('Current session')
  })

  // Regression: 79be608 built every row from `used_percentage` alone and used
  // `resets_at` only to pick the footer text. A 91 % five-hour reading whose
  // window reset ten minutes ago still rendered as a fresh 91 % bar, steering
  // the chooser away from the one subscription that had just got its headroom
  // back.
  it('says nothing about a window whose reset has already passed', () => {
    const expired = usage()
    expired.five_hour.resets_at = Math.floor(NOW.getTime() / 1000) - 600

    render(UsageMeter, { props: { usage: expired } })

    const meter = screen.getByTestId('usage-meter')
    expect(meter).not.toHaveTextContent('5h')
    expect(meter).toHaveTextContent('7d 17%')
    expect(screen.queryByTestId('usage-bar-five-hour')).not.toBeInTheDocument()
  })

  it('renders nothing once every window it had has reset', () => {
    const expired = usage()
    expired.five_hour.resets_at = Math.floor(NOW.getTime() / 1000) - 600
    expired.seven_day.resets_at = Math.floor(NOW.getTime() / 1000) - 60

    const { container } = render(UsageMeter, { props: { usage: expired } })

    expect(container.textContent.trim()).toBe('')
  })

  // Regression: 79be608 read `Date.now()` inside `$derived` with nothing to
  // invalidate it, so a chip that was already on screen when a window reset
  // kept showing the pre-reset percentage until something else re-rendered it.
  it('drops a window as its reset passes, with no new props', async () => {
    render(UsageMeter, { props: { usage: usage() } })
    expect(screen.getByTestId('usage-meter')).toHaveTextContent('5h 26%')

    // Past the five-hour window's reset, three hours out.
    await vi.advanceTimersByTimeAsync(3 * 3600 * 1000 + 60_000)
    await tick()

    expect(screen.getByTestId('usage-meter')).not.toHaveTextContent('5h 26%')
    expect(screen.getByTestId('usage-meter')).toHaveTextContent('7d 17%')
  })

  // Regression: a574720 read `resets_at` through `Number()`, which turns the
  // `null` the IPC layer sends for a window with no reset into 0 — a reset that
  // passed in 1970. The percentage beside it was dropped, so an account whose
  // payload named no reset showed no usage at all.
  it('keeps a window that names no reset, and says when it was seen', () => {
    const withoutReset = usage()
    withoutReset.five_hour.resets_at = null
    withoutReset.seven_day = null

    render(UsageMeter, { props: { usage: withoutReset } })

    const meter = screen.getByTestId('usage-meter')
    expect(meter).toHaveTextContent('5h 26%')
    expect(screen.queryByTestId('usage-reset')).not.toBeInTheDocument()
    expect(screen.getByTestId('usage-observed')).toHaveTextContent('seen 1m ago')
  })

  it('never draws a bar past its track', () => {
    render(UsageMeter, { props: { usage: usage({ fiveHour: 143 }) } })

    expect(screen.getByTestId('usage-bar-five-hour')).toHaveStyle({ width: '100%' })
    expect(screen.getByTestId('usage-meter')).toHaveTextContent('5h 143%')
  })

  it('renders ordered provider windows with their titles and severity', () => {
    // Regression: a574720 shaped the meter around two status-line fields; the
    // OAuth provider supplies ordered, titled windows instead.
    render(UsageMeter, {
      props: {
        tool: 'claude',
        usage: {
          status: 'ok',
          observed_at: new Date(NOW - 60_000).toISOString(),
          windows: [
            {
              key: 'session',
              title: 'Current session',
              used_percentage: 12,
              resets_at: Math.floor((NOW + 3600_000) / 1000),
              severity: 'normal',
              is_active: true,
            },
            {
              key: 'weekly_scoped',
              title: 'Current week (Fable)',
              used_percentage: 82,
              resets_at: Math.floor((NOW + 3 * 86400_000) / 1000),
              severity: 'warning',
              is_active: true,
            },
          ],
        },
      },
    })

    expect(screen.getByText('Current session')).toBeInTheDocument()
    expect(screen.getByText('Current week (Fable)')).toBeInTheDocument()
    expect(screen.getByText('82% used')).toBeInTheDocument()
  })

  it('renders two scoped weekly windows even when a provider repeats its raw kind', () => {
    // Regression: c11770e passed `weekly_scoped` through as every scoped
    // window's key, so Svelte raised each_key_duplicate and removed the meter.
    render(UsageMeter, {
      props: {
        tool: 'claude',
        usage: {
          status: 'ok',
          observed_at: new Date(NOW - 60_000).toISOString(),
          windows: [
            {
              key: 'weekly_scoped',
              title: 'Current week (Fable)',
              used_percentage: 29,
              resets_at: Math.floor((NOW + 3 * 86400_000) / 1000),
              severity: 'normal',
              is_active: true,
            },
            {
              key: 'weekly_scoped',
              title: 'Current week (Opus)',
              used_percentage: 41,
              resets_at: Math.floor((NOW + 3 * 86400_000) / 1000),
              severity: 'warning',
              is_active: true,
            },
          ],
        },
      },
    })

    expect(screen.getByText('Current week (Fable)')).toBeInTheDocument()
    expect(screen.getByText('Current week (Opus)')).toBeInTheDocument()
    expect(screen.getByTestId('usage-bar-weekly_scoped-0')).toBeInTheDocument()
    expect(screen.getByTestId('usage-bar-weekly_scoped-1')).toBeInTheDocument()
  })
})
