import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

const appCss = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8')

describe('mesh canvas theme tokens and animations', () => {
  it('declares mesh design tokens in @theme', () => {
    const requiredTokens = [
      '--mesh-node-gradient-from',
      '--mesh-node-gradient-to',
      '--mesh-node-gradient-hover-from',
      '--mesh-node-gradient-hover-to',
      '--mesh-node-border',
      '--mesh-node-border-hover',
      '--mesh-node-shadow',
      '--mesh-node-shadow-hover',
      '--mesh-lead-border',
      '--mesh-lead-glow',
      '--mesh-lead-gradient-from',
      '--mesh-lead-gradient-to',
      '--mesh-selected-border',
      '--mesh-selected-glow',
      '--mesh-connection-color',
      '--mesh-connection-color-dim',
      '--mesh-connection-width',
      '--mesh-connection-active',
      '--mesh-add-border',
      '--mesh-add-border-hover',
      '--mesh-add-icon',
    ]

    for (const token of requiredTokens) {
      expect(appCss).toContain(token)
    }
  })

  it('declares mesh keyframes and reduced-motion overrides', () => {
    const requiredKeyframes = [
      '@keyframes mesh-draw',
      '@keyframes mesh-established-pulse',
      '@keyframes mesh-connection-breathe',
      '@keyframes mesh-node-enter',
      '@keyframes mesh-node-exit',
      '@keyframes mesh-detail-enter',
    ]
    for (const keyframe of requiredKeyframes) {
      expect(appCss).toContain(keyframe)
    }

    expect(appCss).toContain('@media (prefers-reduced-motion: reduce)')
    expect(appCss).toContain('.mesh-connection')
    expect(appCss).toContain('.mesh-node')
  })
})
