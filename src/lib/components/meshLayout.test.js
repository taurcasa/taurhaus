import { describe, expect, it } from 'vitest'

import { computeMeshLayout } from './meshLayout.js'

function createLead(overrides = {}) {
  return {
    id: 'lead-1',
    name: 'team-lead',
    tool: 'claude',
    model: 'opus',
    status: 'active',
    ...overrides,
  }
}

function createAgents(count) {
  return Array.from({ length: count }, (_, index) => ({
    id: `agent-${index + 1}`,
    name: `agent-${index + 1}`,
    tool: index % 2 === 0 ? 'codex' : 'gemini',
    model: 'gpt-5.4-high',
    status: index % 3 === 0 ? 'active' : 'idle',
  }))
}

function sampleRoute(route, steps = 24) {
  const points = []
  for (let step = 0; step <= steps; step += 1) {
    const t = step / steps
    const mt = 1 - t
    const x = mt ** 3 * route.start.x
      + 3 * mt ** 2 * t * route.control1.x
      + 3 * mt * t ** 2 * route.control2.x
      + t ** 3 * route.end.x
    const y = mt ** 3 * route.start.y
      + 3 * mt ** 2 * t * route.control1.y
      + 3 * mt * t ** 2 * route.control2.y
      + t ** 3 * route.end.y
    points.push({ x, y })
  }
  return points
}

function orientation(a, b, c) {
  const value = (b.y - a.y) * (c.x - b.x) - (b.x - a.x) * (c.y - b.y)
  if (Math.abs(value) < 0.0001) return 0
  return value > 0 ? 1 : 2
}

function onSegment(a, b, c) {
  return Math.min(a.x, c.x) <= b.x
    && b.x <= Math.max(a.x, c.x)
    && Math.min(a.y, c.y) <= b.y
    && b.y <= Math.max(a.y, c.y)
}

function segmentsIntersect(a1, a2, b1, b2) {
  const o1 = orientation(a1, a2, b1)
  const o2 = orientation(a1, a2, b2)
  const o3 = orientation(b1, b2, a1)
  const o4 = orientation(b1, b2, a2)

  if (o1 !== o2 && o3 !== o4) return true
  if (o1 === 0 && onSegment(a1, b1, a2)) return true
  if (o2 === 0 && onSegment(a1, b2, a2)) return true
  if (o3 === 0 && onSegment(b1, a1, b2)) return true
  if (o4 === 0 && onSegment(b1, a2, b2)) return true
  return false
}

function routesCross(left, right) {
  const leftPoints = sampleRoute(left)
  const rightPoints = sampleRoute(right)
  for (let index = 0; index < leftPoints.length - 1; index += 1) {
    for (let inner = 0; inner < rightPoints.length - 1; inner += 1) {
      if (segmentsIntersect(leftPoints[index], leftPoints[index + 1], rightPoints[inner], rightPoints[inner + 1])) {
        return true
      }
    }
  }
  return false
}

function anyRoutesCross(routes) {
  for (let index = 0; index < routes.length; index += 1) {
    for (let inner = index + 1; inner < routes.length; inner += 1) {
      if (routesCross(routes[index], routes[inner])) {
        return true
      }
    }
  }
  return false
}

function assertRoutesWithinBounds(layout, width, height) {
  for (const route of layout.connections) {
    for (const point of [route.start, route.end, route.control1, route.control2, ...sampleRoute(route, 16)]) {
      expect(point.x).toBeGreaterThanOrEqual(0)
      expect(point.x).toBeLessThanOrEqual(width)
      expect(point.y).toBeGreaterThanOrEqual(0)
      expect(point.y).toBeLessThanOrEqual(height)
    }
  }
}

describe('meshLayout', () => {
  it.each([1, 2, 3, 4, 5, 6, 7, 8])('produces exactly one connection per agent for %i agents', (count) => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(count),
    })

    expect(layout.connections).toHaveLength(count)
    expect(new Set(layout.connections.map((route) => route.toId)).size).toBe(count)
    expect(new Set(layout.connections.map((route) => route.start.x)).size).toBe(count)
  })

  it('routes three agents without crossings and gives the center agent a non-degenerate path', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(3),
    })

    expect(anyRoutesCross(layout.connections)).toBe(false)

    const centerRoute = layout.connections.find((route) => route.toId === 'agent-2')
    expect(centerRoute).toBeTruthy()
    expect(Math.abs(centerRoute.control1.x - centerRoute.start.x)).toBeGreaterThan(0)
    expect(Math.abs(centerRoute.control2.x - centerRoute.end.x)).toBeGreaterThan(0)
  })

  it('routes five agents without crossings and keeps the center agent visibly non-degenerate', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(5),
    })

    expect(anyRoutesCross(layout.connections)).toBe(false)

    const centerRoute = layout.connections.find((route) => route.toId === 'agent-3')
    expect(centerRoute).toBeTruthy()
    expect(Math.abs(centerRoute.control1.x - centerRoute.start.x)).toBeGreaterThanOrEqual(8)
    expect(Math.abs(centerRoute.control2.x - centerRoute.end.x)).toBeGreaterThanOrEqual(8)
  })

  it('uses two rows for eight agents and keeps row routes non-crossing', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(8),
    })

    expect(new Set(layout.agents.map((agent) => agent.row))).toEqual(new Set([0, 1]))

    const topRowAgents = layout.agents
      .filter((agent) => agent.row === 0)
      .sort((left, right) => left.x - right.x)
      .map((agent) => agent.id)
    const bottomRowAgents = layout.agents
      .filter((agent) => agent.row === 1)
      .sort((left, right) => left.x - right.x)
      .map((agent) => agent.id)
    const topRowRoutes = layout.connections
      .filter((route) => route.row === 0)
      .sort((left, right) => left.start.x - right.start.x)
    const bottomRowRoutes = layout.connections
      .filter((route) => route.row === 1)
      .sort((left, right) => left.start.x - right.start.x)

    expect(anyRoutesCross(topRowRoutes)).toBe(false)
    expect(anyRoutesCross(bottomRowRoutes)).toBe(false)
    expect(topRowRoutes.map((route) => route.toId)).toEqual(topRowAgents)
    expect(bottomRowRoutes.map((route) => route.toId)).toEqual(bottomRowAgents)
  })

  it('preserves non-crossing ordering after collapsing from seven agents to five', () => {
    const sevenAgentLayout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(7),
    })
    const fiveAgentLayout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(5),
    })

    expect(anyRoutesCross(fiveAgentLayout.connections)).toBe(false)

    const fiveOrder = [...fiveAgentLayout.connections]
      .sort((left, right) => left.start.x - right.start.x)
      .map((route) => route.toId)
    const fiveAgentOrder = [...fiveAgentLayout.agents]
      .sort((left, right) => left.x - right.x)
      .map((agent) => agent.id)

    expect(fiveOrder).toEqual(fiveAgentOrder)
    expect(new Set(sevenAgentLayout.agents.map((agent) => agent.row)).size).toBe(2)
  })

  it('avoids near-zero-width control geometry for all multi-agent layouts', () => {
    for (const count of [2, 3, 4, 5, 6, 7, 8]) {
      const layout = computeMeshLayout({
        width: 960,
        height: 640,
        mode: 'runtime',
        lead: createLead(),
        agents: createAgents(count),
      })

      for (const route of layout.connections) {
        const maxHorizontalDelta = Math.max(
          Math.abs(route.control1.x - route.start.x),
          Math.abs(route.control2.x - route.end.x),
        )
        expect(maxHorizontalDelta).toBeGreaterThanOrEqual(8)
      }
    }
  })

  it('keeps all routes inside the viewBox bounds', () => {
    const width = 960
    const height = 640
    const layout = computeMeshLayout({
      width,
      height,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(8),
    })

    assertRoutesWithinBounds(layout, width, height)
  })

  it('keeps nodes inside narrow containers', () => {
    const width = 360
    const layout = computeMeshLayout({
      width,
      height: 520,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(6),
    })

    expect(layout.lead.x - layout.lead.width / 2).toBeGreaterThanOrEqual(0)
    expect(layout.lead.x + layout.lead.width / 2).toBeLessThanOrEqual(width)

    for (const agent of layout.agents) {
      expect(agent.x - agent.width / 2).toBeGreaterThanOrEqual(0)
      expect(agent.x + agent.width / 2).toBeLessThanOrEqual(width)
    }
  })
})
