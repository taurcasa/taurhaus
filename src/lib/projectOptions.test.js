import { describe, expect, it } from 'vitest'

import { normalizeProjectOption, projectBasename } from './projectOptions.js'

describe('projectOptions', () => {
  it('extracts basename for linux and windows paths', () => {
    expect(projectBasename('/projects/taurhaus')).toBe('taurhaus')
    expect(projectBasename('C:\\work\\taurhaus')).toBe('taurhaus')
  })

  it('normalizes string project values', () => {
    expect(normalizeProjectOption('/projects/taurhaus')).toEqual({
      id: '/projects/taurhaus',
      label: 'taurhaus',
    })
    expect(
      normalizeProjectOption('/projects/taurhaus', {
        stringLabel: 'raw',
      })
    ).toEqual({
      id: '/projects/taurhaus',
      label: '/projects/taurhaus',
    })
  })

  it('normalizes object project values with fallback labels', () => {
    expect(normalizeProjectOption({ path: '/projects/api' })).toEqual({
      id: '/projects/api',
      label: 'api',
    })
    expect(
      normalizeProjectOption(
        { path: '/projects/api' },
        { objectFallbackLabel: 'raw', unnamedLabel: 'Unnamed' }
      )
    ).toEqual({
      id: '/projects/api',
      label: '/projects/api',
    })
    expect(normalizeProjectOption({})).toEqual({
      id: '',
      label: '',
    })
  })
})
