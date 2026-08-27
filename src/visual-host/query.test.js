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

  // Regression: 74c7761 let the screenshot lane address a fixture by URL, and a
  // mistyped scenario fell back to the first one without a word. The shot
  // succeeded on the wrong fixture and was filed as evidence of a bug fix.
  it('says so when the URL asked for a fixture that is not there', () => {
    const bare = { registry: REGISTRY, viewports: VIEWPORTS }

    expect(readVisualHostQuery('?component=nope&scenario=idle', bare).unknownRequest).toBe(true)
    expect(
      readVisualHostQuery('?component=shell-popups&scenario=nope', bare).unknownRequest
    ).toBe(true)
    expect(
      readVisualHostQuery('?component=shell-popups&scenario=chooser-light', bare).unknownRequest
    ).toBe(false)
    // An address that names nothing is not a mistyped one.
    expect(readVisualHostQuery('', bare).unknownRequest).toBe(false)
  })

  // Regression: 74c7761 reported a fallback for the component and the scenario
  // only. A shot of a popup is evidence about its size and its theme too, and
  // `theme=drak` quietly rendered the scenario's own.
  it('says so when the URL asked for a theme or a viewport that is not there', () => {
    const bare = { registry: REGISTRY, viewports: VIEWPORTS }
    const address = '?component=shell-popups&scenario=chooser-light'

    expect(readVisualHostQuery(`${address}&theme=drak`, bare).unknownRequest).toBe(true)
    expect(readVisualHostQuery(`${address}&viewport=huge`, bare).unknownRequest).toBe(true)
    expect(
      readVisualHostQuery(`${address}&viewport=laptop&theme=dark`, bare).unknownRequest
    ).toBe(false)
  })
})
