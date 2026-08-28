import { describe, expect, it } from 'vitest'

import { TOOL_ICONS, getToolIcon, getToolName } from './toolLogos.js'

describe('Antigravity tool visuals', () => {
  it('uses a dedicated monochrome mark and registry label', () => {
    // Regression: 9a66d1c embedded the retired Google CLI sparkle as the harness mark.
    expect(TOOL_ICONS.agy).toBeDefined()
    expect(getToolIcon('agy')).toBe(TOOL_ICONS.agy)
    expect(getToolIcon('agy')).not.toBe(TOOL_ICONS.claude)
    expect(getToolName('agy')).toBe('Antigravity')
  })
})
