import { describe, it, expect } from 'vitest'
import { formatDuration, formatUserFacingError } from './format.js'

describe('formatUserFacingError', () => {
  it('returns error.message when available', () => {
    expect(formatUserFacingError(new Error('boom'), 'fallback')).toBe('boom')
    expect(formatUserFacingError({ message: 'typed message' }, 'fallback')).toBe('typed message')
  })

  it('returns non-empty strings unchanged', () => {
    expect(formatUserFacingError('network unavailable', 'fallback')).toBe('network unavailable')
  })

  it('returns fallback for unknown or empty inputs', () => {
    expect(formatUserFacingError('', 'fallback')).toBe('fallback')
    expect(formatUserFacingError({ code: 'E_FAIL' }, 'fallback')).toBe('fallback')
    expect(formatUserFacingError(null, 'fallback')).toBe('fallback')
  })
})

describe('formatDuration', () => {
  it('returns "< 1m" for durations under 60 seconds', () => {
    expect(formatDuration(0)).toBe('< 1m')
    expect(formatDuration(1000)).toBe('< 1m')
    expect(formatDuration(59_999)).toBe('< 1m')
  })

  it('returns minutes for durations under 1 hour', () => {
    expect(formatDuration(60_000)).toBe('1m')
    expect(formatDuration(5 * 60_000)).toBe('5m')
    expect(formatDuration(59 * 60_000 + 59_999)).toBe('59m')
  })

  it('returns hours and minutes for durations under 24 hours', () => {
    expect(formatDuration(60 * 60_000)).toBe('1h 0m')
    expect(formatDuration(83 * 60_000)).toBe('1h 23m')
    expect(formatDuration(23 * 60 * 60_000 + 59 * 60_000)).toBe('23h 59m')
  })

  it('returns days and hours for durations >= 24 hours', () => {
    expect(formatDuration(24 * 60 * 60_000)).toBe('1d 0h')
    expect(formatDuration(27 * 60 * 60_000)).toBe('1d 3h')
    expect(formatDuration(50 * 60 * 60_000)).toBe('2d 2h')
  })

  it('floors partial minutes', () => {
    // 1 min 30 sec → "1m" (not "1.5m")
    expect(formatDuration(90_000)).toBe('1m')
  })
})
