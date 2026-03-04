import { describe, it, expect } from 'vitest'
import { getBatchProgressName } from './batchRegistrationProgress.js'

describe('getBatchProgressName', () => {
  it('prefers camelCase projectName when present', () => {
    expect(getBatchProgressName({ projectName: 'alpha', project_name: 'legacy' })).toBe('alpha')
  })

  it('falls back to legacy snake_case project_name', () => {
    expect(getBatchProgressName({ project_name: 'legacy-only' })).toBe('legacy-only')
  })

  it('returns empty string for invalid payloads', () => {
    expect(getBatchProgressName(null)).toBe('')
    expect(getBatchProgressName(undefined)).toBe('')
    expect(getBatchProgressName('invalid')).toBe('')
  })
})
