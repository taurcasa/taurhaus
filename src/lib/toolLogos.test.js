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

  it('gives Grok its own mark rather than the unknown-tool fallback', () => {
    // Regression: commit 8fcb5b3 registered grok without a mark, so every grok
    // session rendered the neutral question-mark glyph.
    expect(TOOL_ICONS.grok).toBeDefined()
    expect(getToolIcon('grok')).toBe(TOOL_ICONS.grok)
    expect(getToolIcon('grok')).not.toBe(TOOL_ICONS.unknown)
    expect(getToolIcon('grok', 'sidebarSmall')).not.toBe(TOOL_ICONS.grok)
    expect(getToolName('grok')).toBe('Grok')
  })

  it('uses a neutral mark and label for an unknown persisted tool', () => {
    // Regression: commit 91f4d3f7 made unknown registry values visually
    // indistinguishable from Claude after a persisted-tool migration.
    expect(getToolIcon('unknown')).toBe(TOOL_ICONS.unknown)
    expect(getToolIcon('unknown')).not.toBe(TOOL_ICONS.claude)
    expect(getToolName('unknown')).toBe('Unknown tool')
  })
})
