import { describe, expect, it } from 'vitest'

import { getRegistryEntry, visualRegistry, viewportPresets } from './registry.js'

describe('visual host registry', () => {
  it('registers all five target components', () => {
    expect(visualRegistry.map((entry) => entry.id)).toEqual([
      'mesh-canvas',
      'hover-card',
      'mesh-node-detail',
      'mesh-team-builder',
      'sidebar',
    ])
  })

  it('exposes at least one scenario per component', () => {
    for (const entry of visualRegistry) {
      expect(entry.label.length).toBeGreaterThan(0)
      expect(entry.component).toBeTruthy()
      expect(Array.isArray(entry.scenarios)).toBe(true)
      expect(entry.scenarios.length).toBeGreaterThan(0)
      expect(entry.scenarios.every((scenario) => typeof scenario.name === 'string')).toBe(true)
    }
  })

  it('exports the required viewport presets', () => {
    expect(viewportPresets.map((preset) => preset.id)).toEqual(['desktop', 'laptop', 'narrow'])
  })

  it('falls back to the first entry for unknown component ids', () => {
    expect(getRegistryEntry('missing-id').id).toBe('mesh-canvas')
  })
})
