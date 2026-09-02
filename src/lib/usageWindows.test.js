import { describe, expect, it } from 'vitest'

import {
  compactSelection,
  exhaustedUsage,
  resetLabel,
  windowPressure,
} from './usageWindows.js'

/** A fixed clock: a window is only spent while it is still the live one. */
const NOW = Date.parse('2026-08-30T09:00:00Z')
const inSeconds = (seconds) => Math.floor(NOW / 1000) + seconds

/** One provider window in the shape `list_accounts` normalises to. */
function window(key, title, usedPercentage, extra = {}) {
  return {
    key,
    title,
    used_percentage: usedPercentage,
    resets_at: inSeconds(2 * 24 * 3600),
    severity: 'normal',
    is_active: true,
    ...extra,
  }
}

function snapshot(status, windows) {
  return { observed_at: '2026-08-30T09:00:00Z', status, windows, note: null }
}

describe('compactSelection', () => {
  it('prefers the windows the provider flagged', () => {
    const flagged = window('week', 'Current week', 40, { compact: true })
    expect(compactSelection([window('session', 'Current session', 10), flagged])).toEqual([
      flagged,
    ])
  })
})

describe('exhaustedUsage', () => {
  it('reports nothing when there is nothing to report', () => {
    expect(exhaustedUsage(null, NOW)).toBe(null)
    expect(exhaustedUsage(undefined, NOW)).toBe(null)
    expect(exhaustedUsage(snapshot('ok', []), NOW)).toBe(null)
  })

  it('reports the window that is spent, at exactly 100', () => {
    const spent = window('week', 'Current week (all models)', 100)

    expect(
      exhaustedUsage(snapshot('ok', [window('session', 'Current session', 62), spent]), NOW)
    ).toEqual({ kind: 'exhausted', window: spent })
  })

  it('reports the first spent window in the provider order', () => {
    const first = window('session', 'Current session', 100)
    const second = window('week', 'Current week (all models)', 104)

    expect(exhaustedUsage(snapshot('ok', [first, second]), NOW)).toEqual({
      kind: 'exhausted',
      window: first,
    })
  })

  it('counts a stale snapshot — it is the last thing known', () => {
    const spent = window('week', 'Current week (all models)', 100)

    expect(exhaustedUsage(snapshot('stale', [spent]), NOW)).toEqual({
      kind: 'exhausted',
      window: spent,
    })
  })

  it('says an unauthorized account needs signing in, whatever its windows say', () => {
    expect(
      exhaustedUsage(snapshot('unauthorized', [window('week', 'Current week', 3)]), NOW)
    ).toEqual({ kind: 'unauthorized' })
  })

  it('reports nothing for a provider that does not measure usage', () => {
    expect(exhaustedUsage(snapshot('unsupported', []), NOW)).toBe(null)
    expect(exhaustedUsage(snapshot('unsupported', [window('week', 'Current week', 100)]), NOW)).toBe(
      null
    )
  })

  it('reports nothing while every window still has headroom', () => {
    expect(
      exhaustedUsage(
        snapshot('ok', [window('session', 'Current session', 99.4), window('week', 'Current week', 12)]),
        NOW
      )
    ).toBe(null)
  })

  it('ignores a window whose percentage is not a number', () => {
    expect(exhaustedUsage(snapshot('ok', [window('week', 'Current week', null)]), NOW)).toBe(null)
    expect(exhaustedUsage({ status: 'ok', windows: null }, NOW)).toBe(null)
  })

  // The rule `UsageMeter` draws by: a window past its own reset has come back,
  // so a reading taken before the reset says nothing about what is left now.
  it('ignores a limit whose reset has already passed', () => {
    const gone = window('week', 'Current week', 100, { resets_at: inSeconds(-60) })

    expect(exhaustedUsage(snapshot('stale', [gone]), NOW)).toBe(null)
  })

  it('still reports a later window that has not reset yet', () => {
    const gone = window('session', 'Current session', 100, { resets_at: inSeconds(-60) })
    const live = window('week', 'Current week', 100)

    expect(exhaustedUsage(snapshot('ok', [gone, live]), NOW)).toEqual({
      kind: 'exhausted',
      window: live,
    })
  })

  it('reports a spent window that names no reset at all', () => {
    const spent = window('week', 'Current week', 100, { resets_at: null })

    expect(exhaustedUsage(snapshot('ok', [spent]), NOW)).toEqual({
      kind: 'exhausted',
      window: spent,
    })
  })
})

describe('resetLabel', () => {
  it('has nothing to say about a window that never resets', () => {
    expect(resetLabel(null, NOW)).toBe(null)
    expect(resetLabel(undefined, NOW)).toBe(null)
    expect(resetLabel('not a time', NOW)).toBe(null)
  })

  it('names the day only once the reset is more than a day out', () => {
    // Locale-independent: a weekday is a word, a clock time is not.
    expect(resetLabel(inSeconds(3 * 3600), NOW)).not.toMatch(/^\p{L}/u)
    expect(resetLabel(inSeconds(40 * 3600), NOW)).toMatch(/^\p{L}/u)
  })

  it('always carries the clock time', () => {
    expect(resetLabel(inSeconds(3 * 3600), NOW)).toMatch(/\d{1,2}:\d{2}/)
    expect(resetLabel(inSeconds(40 * 3600), NOW)).toMatch(/\d{1,2}:\d{2}/)
  })
})

describe('windowPressure', () => {
  // Regression: 186f19a2 let the meter bar read provider severity while the
  // account row read percentage, so one window could be amber in one surface
  // and emerald in the other.
  it('is one reading of severity and percentage together', () => {
    expect(windowPressure({ used_percentage: 50, severity: 'warning' })).toBe('warning')
    expect(windowPressure({ used_percentage: 95, severity: 'critical' })).toBe('critical')
    expect(windowPressure({ used_percentage: 85, severity: 'normal' })).toBe('warning')
    expect(windowPressure({ used_percentage: 100 })).toBe('critical')
    expect(windowPressure({ usedPercentage: 12 })).toBe('normal')
    expect(windowPressure(null)).toBe('normal')
  })
})
