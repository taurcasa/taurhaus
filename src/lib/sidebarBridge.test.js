import { describe, expect, it } from 'vitest'

import {
  BRIDGE_FLARE_PX,
  BRIDGE_MAX_LANE_PX,
  BRIDGE_PANEL_GAP_PX,
  RAIL_BORDER_PX,
  bridgeFrame,
} from './sidebarBridge.js'

// The Shell body geometry the frames below describe: the rail (aside) floats
// at x 6..258 with a 1px border, the frame gutter runs 258..264, the main
// panel starts at 264. The list viewport is inset inside the rail.
function rect({ left, top, right, bottom }) {
  return { left, top, right, bottom, width: right - left, height: bottom - top }
}

const railRect = rect({ left: 6, top: 46, right: 258, bottom: 634 })
const listRect = rect({ left: 7, top: 92, right: 257, bottom: 590 })
const panelLeft = 264

// A pulled row flush against the rail's inner border edge (no scrollbar).
function flushRowRect({ top = 200, height = 36 } = {}) {
  return rect({ left: 13, top, right: 257, bottom: top + height })
}

function frame(overrides = {}) {
  return bridgeFrame({
    rowRect: flushRowRect(),
    railRect,
    listRect,
    panelLeft,
    scrollTop: 0,
    scrollHeight: 498,
    clientHeight: 498,
    ...overrides,
  })
}

describe('bridgeFrame — show/hide', () => {
  it('is inactive without a pulled row (held state, or none selected)', () => {
    expect(frame({ rowRect: null }).active).toBe(false)
  })

  it('is inactive without rail or list geometry', () => {
    expect(frame({ railRect: null }).active).toBe(false)
    expect(frame({ listRect: null }).active).toBe(false)
  })

  it('is inactive when the list viewport has collapsed', () => {
    const collapsed = rect({ left: 7, top: 92, right: 257, bottom: 92 })
    expect(frame({ listRect: collapsed }).active).toBe(false)
  })

  it('is inactive when the row stops short by more than a scrollbar lane', () => {
    // Wider than any scrollbar we know: geometry we don't understand, so the
    // material honestly stays unbridged rather than floating beside the rail.
    const shortRow = rect({ left: 13, top: 200, right: 257 - BRIDGE_MAX_LANE_PX - 3, bottom: 236 })
    expect(frame({ rowRect: shortRow }).active).toBe(false)
  })

  it('is inactive when the row overhangs the rail edge', () => {
    const overhang = rect({ left: 13, top: 200, right: 260, bottom: 236 })
    expect(frame({ rowRect: overhang }).active).toBe(false)
  })

  it('is inactive when the panel is not to the right of the rail', () => {
    expect(frame({ panelLeft: railRect.right }).active).toBe(false)
  })
})

describe('bridgeFrame — gutter wrapper and strip', () => {
  it('spans from the rail border to the panel edge, clipped to the list viewport', () => {
    const { wrapper } = frame()
    expect(wrapper.left).toBe(railRect.right - RAIL_BORDER_PX)
    expect(wrapper.width).toBe(panelLeft - (railRect.right - RAIL_BORDER_PX))
    expect(wrapper.top).toBe(listRect.top)
    expect(wrapper.height).toBe(listRect.height)
  })

  it('falls back to the frame-gap token when the panel cannot be measured', () => {
    const { wrapper } = frame({ panelLeft: null })
    expect(wrapper.width).toBe(BRIDGE_PANEL_GAP_PX + RAIL_BORDER_PX)
  })

  it('flares the strip beyond the row to meet the rail scoop tips', () => {
    const { strip } = frame()
    expect(strip.top).toBe(200 - listRect.top - BRIDGE_FLARE_PX)
    expect(strip.height).toBe(36 + 2 * BRIDGE_FLARE_PX)
  })

  it('states the strip base in content coordinates so scrolling cancels out', () => {
    // The same row seen 120px into a scroll must produce the same base: the
    // strip is positioned at content offset and translated by -scrollTop.
    const scrolled = frame({
      rowRect: flushRowRect({ top: 200 - 120 }),
      scrollTop: 120,
      scrollHeight: 900,
      clientHeight: 498,
    })
    expect(scrolled.strip.top).toBe(frame().strip.top)
  })

  it('reports the scroll range for the compositor-driven follower', () => {
    expect(frame({ scrollHeight: 900, clientHeight: 498 }).scrollRange).toBe(402)
    expect(frame().scrollRange).toBe(0)
  })
})

describe('bridgeFrame — scrollbar lane', () => {
  const laneRow = rect({ left: 13, top: 200, right: 249, bottom: 236 })

  it('is absent when the row is flush with the rail edge', () => {
    expect(frame().lane).toBe(null)
  })

  it('tolerates sub-pixel shortfall without inventing a lane', () => {
    const nearFlush = rect({ left: 13, top: 200, right: 256.6, bottom: 236 })
    expect(frame({ rowRect: nearFlush }).lane).toBe(null)
  })

  it('covers the scrollbar lane in rail coordinates when the row stops short', () => {
    const { lane } = frame({ rowRect: laneRow, scrollHeight: 900 })
    expect(lane).not.toBe(null)
    // Rail-relative x: from the row's right edge to the rail's inner border.
    expect(lane.left).toBe(laneRow.right - railRect.left)
    expect(lane.width).toBe(railRect.right - RAIL_BORDER_PX - laneRow.right)
    expect(lane.top).toBe(listRect.top - railRect.top)
    expect(lane.height).toBe(listRect.height)
  })

  it('gives the lane strip the same vertical base as the gutter strip', () => {
    const result = frame({ rowRect: laneRow, scrollHeight: 900 })
    expect(result.lane.strip).toEqual(result.strip)
  })
})
