import { describe, expect, it, vi } from 'vitest'
import { commands } from 'vitest/browser'

// The bridge scenarios carry their own session-free project lists; the
// store is stubbed so Sidebar renders without the app's polling machinery.
vi.mock('../../../lib/sessionStore.svelte.js', () => ({
  getSessionForProject: vi.fn(() => null),
  getSessionsForProject: vi.fn(() => []),
}))

import SidebarBridgeHost from '../../../visual-host/hosts/SidebarBridgeHost.svelte'
import { captureVisualBase64, renderVisual } from '../renderVisual.js'
import { sidebarBridgeScenarios } from '../fixtures/sidebarBridge.fixtures.js'

const viewport = { width: 960, height: 640 }

function frame() {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

/**
 * Whether this engine's scrollbars take layout space. The product engine
 * (WebView2 on Windows) uses classic space-taking scrollbars — there the
 * in-rail lane cover appears when the list overflows. Linux HeadlessChrome
 * uses overlay scrollbars unconditionally (styling included), so the pulled
 * row stays flush and no lane exists to cover; the `just visual-shot` Edge
 * lane is where the lane is judged on product-faithful scrollbars.
 */
function scrollbarTakesSpace() {
  const probe = document.createElement('div')
  probe.style.cssText =
    'position:absolute;visibility:hidden;width:100px;height:100px;overflow-y:scroll'
  document.body.appendChild(probe)
  const takes = probe.offsetWidth - probe.clientWidth > 0
  probe.remove()
  return takes
}

async function renderScenario(scenario) {
  await renderVisual(SidebarBridgeHost, {
    theme: scenario.theme,
    viewport,
    props: { scenario, theme: scenario.theme },
  })
  // Host applies scrollTo in onMount; give the scroll event and the bridge
  // driver's effect a couple of frames to land.
  await frame()
  await frame()

  return {
    rail: document.querySelector('aside'),
    list: document.querySelector('[data-testid="sidebar-project-scroll"]'),
    panel: document.querySelector('main.shell-main-panel'),
    bridge: document.querySelector('[data-testid="sidebar-bridge"]'),
    lane: document.querySelector('[data-testid="sidebar-bridge-lane"]'),
    row: document.querySelector('.sidebar-row-pulled'),
  }
}

describe('Pulled-row bridge — Shell body junction', () => {
  it.each(sidebarBridgeScenarios)('captures $name', async (scenario) => {
    const { rail, list, panel, bridge, lane, row } = await renderScenario(scenario)

    expect(rail).toBeTruthy()
    expect(panel).toBeTruthy()
    expect(bridge).toBeTruthy()

    if (scenario.expected.bridge) {
      expect(bridge.hasAttribute('data-bridge-active')).toBe(true)
      expect(row).toBeTruthy()

      const railRect = rail.getBoundingClientRect()
      const listRect = list.getBoundingClientRect()
      const panelRect = panel.getBoundingClientRect()
      const bridgeRect = bridge.getBoundingClientRect()
      const rowRect = row.getBoundingClientRect()

      // The clip box: rail border → panel edge plus the 2px hairline cover,
      // tall as the list viewport, so overflow clips the strip on the same
      // lines that clip the row.
      expect(bridgeRect.left).toBeCloseTo(railRect.right - 1, 0)
      expect(bridgeRect.right).toBeCloseTo(panelRect.left + 2, 0)
      expect(bridgeRect.top).toBeCloseTo(listRect.top, 0)
      expect(bridgeRect.height).toBeCloseTo(listRect.height, 0)

      // The strip tracks the row: flared 8px to the scoop tips. Its rect
      // includes the -scrollTop translation, so this holds scrolled too.
      const strip = bridge.querySelector('.sidebar-bridge-strip')
      const stripRect = strip.getBoundingClientRect()
      expect(stripRect.top).toBeCloseTo(rowRect.top - 8, 0)
      expect(stripRect.height).toBeCloseTo(rowRect.height + 16, 0)

      if (scenario.expected.clippedTop) {
        // The scenario's point: the row is half out at the list's top edge.
        expect(rowRect.top).toBeLessThan(listRect.top)
      }

      if (scenario.expected.lane && scrollbarTakesSpace()) {
        // The classic scrollbar took its lane; the in-rail cover spans from
        // the row's short edge to the rail border, under the thumb.
        expect(lane.hasAttribute('data-bridge-active')).toBe(true)
        const laneRect = lane.getBoundingClientRect()
        expect(rowRect.right).toBeLessThan(railRect.right - 2)
        expect(laneRect.left).toBeCloseTo(rowRect.right, 0)
        expect(laneRect.right).toBeCloseTo(railRect.right - 1, 0)
      } else {
        // Overlay-scrollbar engines (and short lists): the row is flush and
        // there is no lane to cover.
        expect(rowRect.right).toBeCloseTo(railRect.right - 1, 0)
        expect(lane.hasAttribute('data-bridge-active')).toBe(false)
      }
    } else {
      expect(bridge.hasAttribute('data-bridge-active')).toBe(false)
      expect(lane.hasAttribute('data-bridge-active')).toBe(false)
      // Held, not unselected: the material lives on the footer key instead.
      expect(row).toBe(null)
    }

    const screenshotPath = `sidebar-bridge/${scenario.name}.png`
    await captureVisualBase64(screenshotPath, {
      clip: { x: 0, y: 0, ...viewport },
    })
    const artifact = await commands.readVisualArtifact(screenshotPath)
    expect(artifact.isPng).toBe(true)
    expect(artifact.size).toBeGreaterThan(0)
  })

  it.each([
    ['compositor', sidebarBridgeScenarios.find((s) => s.name === 'bridge_scrolled_lane_dark')],
    ['js-fallback', sidebarBridgeScenarios.find((s) => s.name === 'bridge_js_tracking_dark')],
  ])('tracks the row through a smooth scroll without shear (%s)', async (mode, scenario) => {
    const { list, bridge, row } = await renderScenario(scenario)
    const strip = bridge.querySelector('.sidebar-bridge-strip')

    // Sample the strip-vs-row offset every frame across an animated scroll:
    // the two must move as one surface. (30 projects stay under the
    // virtualization threshold, so the row never unmounts mid-scroll.)
    const divergence = []
    list.scrollTo({ top: 420, behavior: 'smooth' })
    for (let i = 0; i < 40; i += 1) {
      await frame()
      const rowTop = row.getBoundingClientRect().top
      const stripTop = strip.getBoundingClientRect().top
      divergence.push(Math.abs(rowTop - 8 - stripTop))
    }

    expect(list.scrollTop).toBeCloseTo(420, 0)
    expect(Math.max(...divergence)).toBeLessThan(1)

    // One frame from a second pass in flight, for the eyeball record.
    list.scrollTo({ top: 120, behavior: 'smooth' })
    await frame()
    await captureVisualBase64(`sidebar-bridge/tracking_mid_scroll_${mode}.png`, {
      clip: { x: 0, y: 0, ...viewport },
    })
  })
})
