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

  it('uses a neutral mark and label for an unknown persisted tool', () => {
    // Regression: commit 91f4d3f7 made unknown registry values visually
    // indistinguishable from Claude after a persisted-tool migration.
    expect(getToolIcon('unknown')).toBe(TOOL_ICONS.unknown)
    expect(getToolIcon('unknown')).not.toBe(TOOL_ICONS.claude)
    expect(getToolName('unknown')).toBe('Unknown tool')
  })
})
