/**
 * The visual host's URL, read as a fixture address.
 *
 * A screenshot lane cannot click three selects, so `?component=&scenario=&
 * viewport=&theme=` names the state to render and `?chrome=0` hides the
 * controls around it. Every part is optional and every unknown value falls back
 * to what the host would have shown on its own — a mistyped scenario should
 * still render something, not a blank page.
 */

const THEMES = new Set(['light', 'dark'])

function firstOf(list) {
  return Array.isArray(list) ? list[0] : undefined
}

export function readVisualHostQuery(search, { registry = [], viewports = [] } = {}) {
  const params = new URLSearchParams(String(search ?? ''))

  const requestedComponent = String(params.get('component') ?? '').trim()
  const entry =
    registry.find((candidate) => candidate.id === requestedComponent) ?? firstOf(registry) ?? null

  const requestedScenario = String(params.get('scenario') ?? '').trim()
  const scenario =
    entry?.scenarios?.find((candidate) => candidate.name === requestedScenario) ??
    entry?.scenarios?.[0] ??
    null

  const requestedViewport = String(params.get('viewport') ?? '').trim()
  const viewport =
    viewports.find((candidate) => candidate.id === requestedViewport) ?? firstOf(viewports) ?? null

  const requestedTheme = String(params.get('theme') ?? '').trim().toLowerCase()
  const themePinned = THEMES.has(requestedTheme)

  return {
    componentId: entry?.id ?? '',
    scenarioName: scenario?.name ?? '',
    viewportId: viewport?.id ?? 'desktop',
    theme: themePinned ? requestedTheme : (scenario?.theme ?? 'light'),
    themePinned,
    // `chrome=0` is the only way to hide the controls; anything else keeps them.
    chrome: params.get('chrome') !== '0',
  }
}
