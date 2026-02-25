import { describe, it, expect } from 'vitest'
import { themeTokens } from './themeTokens.js'

describe('themeTokens', () => {
  it('returns an object with all expected token keys', () => {
    const t = themeTokens(false)
    const expectedKeys = [
      'textPrimary', 'textSecondary', 'textTertiary', 'textMuted', 'textBody',
      'mainBg', 'cardBg', 'sectionBg', 'listBg',
      'keyline',
      'hoverRow', 'listHover', 'listSelected', 'fileBg',
      'linkColor', 'hashColor', 'questionMark',
      'inputBg', 'checkBg', 'labelColor',
    ]
    for (const key of expectedKeys) {
      expect(t).toHaveProperty(key)
      expect(typeof t[key]).toBe('string')
      expect(t[key].length).toBeGreaterThan(0)
    }
  })

  it('returns light mode values when dark=false', () => {
    const t = themeTokens(false)
    expect(t.textPrimary).toBe('text-zinc-900')
    expect(t.textBody).toBe('text-zinc-700')
    expect(t.mainBg).toBe('bg-white')
    expect(t.keyline).toBe('border-zinc-200')
    expect(t.linkColor).toContain('text-brand-600')
  })

  it('returns dark mode values when dark=true', () => {
    const t = themeTokens(true)
    expect(t.textPrimary).toBe('text-zinc-100')
    expect(t.textBody).toBe('text-zinc-300')
    expect(t.mainBg).toBe('bg-zinc-950')
    expect(t.keyline).toBe('border-zinc-800')
    expect(t.linkColor).toContain('text-brand-400')
  })

  it('textTertiary is the same in both modes', () => {
    expect(themeTokens(false).textTertiary).toBe(themeTokens(true).textTertiary)
    expect(themeTokens(false).textTertiary).toBe('text-zinc-500')
  })

  it('all values are non-empty strings', () => {
    for (const dark of [true, false]) {
      const t = themeTokens(dark)
      for (const [key, val] of Object.entries(t)) {
        expect(val, `${key} (dark=${dark})`).toBeTruthy()
        expect(typeof val, `${key} (dark=${dark})`).toBe('string')
      }
    }
  })
})
