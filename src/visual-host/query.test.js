import { describe, expect, it } from 'vitest'

import { readVisualHostQuery } from './query.js'

const REGISTRY = [
  { id: 'mesh-canvas', scenarios: [{ name: 'idle', theme: 'light' }] },
  {
    id: 'shell-popups',
    scenarios: [
      { name: 'chooser-light', theme: 'light' },
      { name: 'chip-menu-dark', theme: 'dark' },
    ],
  },
]
const VIEWPORTS = [
  { id: 'desktop', width: 1920, height: 1080 },
  { id: 'laptop', width: 1366, height: 768 },
]

describe('readVisualHostQuery', () => {
  it('addresses a fixture by component, scenario, viewport, and theme', () => {
    const query = readVisualHostQuery(
      '?component=shell-popups&scenario=chip-menu-dark&viewport=laptop&theme=dark&chrome=0',
      { registry: REGISTRY, viewports: VIEWPORTS }
    )

    expect(query).toMatchObject({
      componentId: 'shell-popups',
      scenarioName: 'chip-menu-dark',
      viewportId: 'laptop',
      theme: 'dark',
      themePinned: true,
      chrome: false,
    })
  })

  it('keeps the chrome unless the URL asks for it to go', () => {
    const bare = { registry: REGISTRY, viewports: VIEWPORTS }
    expect(readVisualHostQuery('', bare).chrome).toBe(true)
    expect(readVisualHostQuery('?chrome=1', bare).chrome).toBe(true)
    expect(readVisualHostQuery('?chrome=0', bare).chrome).toBe(false)
  })

  it('takes the scenario theme when the URL names none', () => {
    const query = readVisualHostQuery('?component=shell-popups&scenario=chip-menu-dark', {
      registry: REGISTRY,
      viewports: VIEWPORTS,
    })

    expect(query.theme).toBe('dark')
    expect(query.themePinned).toBe(false)
  })

  it('falls back to the first entry rather than rendering nothing', () => {
    const query = readVisualHostQuery('?component=nope&scenario=nope&viewport=nope&theme=puce', {
      registry: REGISTRY,
      viewports: VIEWPORTS,
    })

    expect(query).toMatchObject({
      componentId: 'mesh-canvas',
      scenarioName: 'idle',
      viewportId: 'desktop',
      theme: 'light',
      themePinned: false,
    })
  })
})
