import { describe, expect, it } from 'vitest'

import { tools } from './toolRegistry.js'
import { TOOL_ICONS, getToolIcon, getToolName } from './toolLogos.js'

describe('brand marks', () => {
  it('draws Grok as the real xAI swoosh at both sizes', () => {
    // Regression: commit 6be3761 shipped a hand-drawn double-slash letterform
    // as the Grok mark — it looked nothing like the mark Grok users know.
    // The real mark is the two-path swoosh from thesvg.org, 1024x1024.
    expect(TOOL_ICONS.grok.viewBox).toBe('0 0 1024 1024')
    expect(TOOL_ICONS.grok.path.startsWith('M395.479 633.828')).toBe(true)
    expect(TOOL_ICONS.grok.path).toContain('M325.226 695.251')

    const small = getToolIcon('grok', 'sidebarSmall')
    expect(small.viewBox).toBe(TOOL_ICONS.grok.viewBox)
    expect(small.path).toBe(TOOL_ICONS.grok.path)
  })

  it('draws Antigravity as the real arch silhouette at both sizes', () => {
    // Regression: commit 8e68468 shipped a hand-drawn orbit mark for
    // Antigravity. The real mark is Google's arch silhouette, 24x24.
    expect(TOOL_ICONS.agy.viewBox).toBe('0 0 24 24')
    expect(TOOL_ICONS.agy.path.startsWith('M21.751 22.607')).toBe(true)

    const small = getToolIcon('agy', 'sidebarSmall')
    expect(small.viewBox).toBe(TOOL_ICONS.agy.viewBox)
    expect(small.path).toBe(TOOL_ICONS.agy.path)
  })

  it('leaves the Claude and Codex marks alone', () => {
    expect(TOOL_ICONS.claude.viewBox).toBe('0 0 16 16')
    expect(TOOL_ICONS.codex.viewBox).toBe('0 0 16 16')
  })

  it('gives every registered tool both an icon and a sidebar variant', () => {
    // Regression: commit 8fcb5b3 added a tool to the registry without a mark,
    // so its sessions rendered the neutral question-mark glyph.
    for (const tool of tools()) {
      const icon = getToolIcon(tool.id)
      const small = getToolIcon(tool.id, 'sidebarSmall')
      expect(icon, `${tool.id} default mark`).not.toBe(TOOL_ICONS.unknown)
      expect(small, `${tool.id} sidebar mark`).toBeDefined()
      for (const variant of [icon, small]) {
        expect(variant.viewBox).toMatch(/^0 0 \d+ \d+$/)
        expect(variant.path.startsWith('M')).toBe(true)
      }
    }
  })
})

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
