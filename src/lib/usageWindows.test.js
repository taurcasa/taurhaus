import { describe, expect, it } from 'vitest'

import { compactSelection, exhaustedUsage, resetLabel } from './usageWindows.js'

/** One provider window in the shape `list_accounts` normalises to. */
function window(key, title, usedPercentage, extra = {}) {
  return {
    key,
    title,
    used_percentage: usedPercentage,
    resets_at: 1788300000,
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
    expect(exhaustedUsage(null)).toBe(null)
    expect(exhaustedUsage(undefined)).toBe(null)
    expect(exhaustedUsage(snapshot('ok', []))).toBe(null)
  })

  it('reports the window that is spent, at exactly 100', () => {
    const spent = window('week', 'Current week (all models)', 100)

    expect(exhaustedUsage(snapshot('ok', [window('session', 'Current session', 62), spent]))).toEqual(
      { kind: 'exhausted', window: spent }
    )
  })

  it('reports the first spent window in the provider order', () => {
    const first = window('session', 'Current session', 100)
    const second = window('week', 'Current week (all models)', 104)

    expect(exhaustedUsage(snapshot('ok', [first, second]))).toEqual({
      kind: 'exhausted',
      window: first,
    })
  })

  it('counts a stale snapshot — it is the last thing known', () => {
    const spent = window('week', 'Current week (all models)', 100)

    expect(exhaustedUsage(snapshot('stale', [spent]))).toEqual({ kind: 'exhausted', window: spent })
  })

  it('says an unauthorized account needs signing in, whatever its windows say', () => {
    expect(exhaustedUsage(snapshot('unauthorized', [window('week', 'Current week', 3)]))).toEqual({
      kind: 'unauthorized',
    })
  })

  it('reports nothing for a provider that does not measure usage', () => {
    expect(exhaustedUsage(snapshot('unsupported', []))).toBe(null)
    expect(exhaustedUsage(snapshot('unsupported', [window('week', 'Current week', 100)]))).toBe(null)
  })

  it('reports nothing while every window still has headroom', () => {
    expect(
      exhaustedUsage(snapshot('ok', [window('session', 'Current session', 99.4), window('week', 'Current week', 12)]))
    ).toBe(null)
  })

  it('ignores a window whose percentage is not a number', () => {
    expect(exhaustedUsage(snapshot('ok', [window('week', 'Current week', null)]))).toBe(null)
    expect(exhaustedUsage({ status: 'ok', windows: null })).toBe(null)
  })
})

describe('resetLabel', () => {
  const NOW = Date.parse('2026-08-30T09:00:00Z')
  const inSeconds = (seconds) => Math.floor(NOW / 1000) + seconds

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
