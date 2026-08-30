import { describe, expect, it } from 'vitest'

import { computeMeshLayout, RUN_TREE_METRICS } from './meshLayout.js'

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
    tool: index % 2 === 0 ? 'codex' : 'agy',
    model: 'gpt-5.4 high',
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
  it('makes room for an opaque-wrapper sentence and anchors routes to the taller node', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(1).map((agent) => ({
        ...agent,
        accountApplied: false,
        accountNote: 'opaque_base_command',
        accountNoteDetail: 'team-wrapper',
      })),
    })

    expect(layout.agents[0].height).toBe(82)
    expect(layout.connections[0].end.y).toBe(layout.agents[0].y - 41)
  })

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

describe('meshLayout run tree child', () => {
  function layoutWithTree(runTree, overrides = {}) {
    return computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: [{ ...createAgents(1)[0], runTree }],
      ...overrides,
    })
  }

  it('places no box for a node without a run', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(2),
    })

    expect(layout.lead.runTree).toBeNull()
    expect(layout.agents.every((agent) => agent.runTree === null)).toBe(true)
  })

  it('hangs the box under its node, centred on it', () => {
    const layout = layoutWithTree({ rowCount: 4 })
    const [agent] = layout.agents

    expect(agent.runTree.top).toBe(agent.y + agent.height / 2 + RUN_TREE_METRICS.gap)
    expect(agent.runTree.left + agent.runTree.width / 2).toBeCloseTo(agent.x, 5)
  })

  it('sizes the box from the row count', () => {
    const four = layoutWithTree({ rowCount: 4 }).agents[0].runTree
    const two = layoutWithTree({ rowCount: 2 }).agents[0].runTree

    expect(four.height - two.height).toBe(2 * RUN_TREE_METRICS.rowHeight)
    expect(four.rowCount).toBe(4)
  })

  it('gives a collapsed run a header-only box', () => {
    const collapsed = layoutWithTree({ rowCount: 0, collapsed: true }).agents[0].runTree

    expect(collapsed.collapsed).toBe(true)
    expect(collapsed.height).toBe(
      RUN_TREE_METRICS.paddingY * 2 + RUN_TREE_METRICS.headerHeight
    )
  })

  it('never renders a box narrower than a readable minimum', () => {
    const layout = computeMeshLayout({
      width: 360,
      height: 520,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(6).map((agent) => ({ ...agent, runTree: { rowCount: 3 } })),
    })

    for (const agent of layout.agents) {
      expect(agent.runTree.width).toBeGreaterThanOrEqual(RUN_TREE_METRICS.minWidth)
    }
  })

  it('keeps the box inside the canvas', () => {
    const width = 420
    const layout = computeMeshLayout({
      width,
      height: 520,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(3).map((agent) => ({ ...agent, runTree: { rowCount: 5 } })),
    })

    for (const agent of layout.agents) {
      expect(agent.runTree.left).toBeGreaterThanOrEqual(0)
      expect(agent.runTree.left + agent.runTree.width).toBeLessThanOrEqual(width)
    }
  })

  it('places a box under the lead too', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: { ...createLead(), runTree: { rowCount: 2 } },
      agents: createAgents(2),
    })

    expect(layout.lead.runTree.top).toBe(
      layout.lead.y + layout.lead.height / 2 + RUN_TREE_METRICS.gap
    )
  })

  it('ignores a malformed descriptor', () => {
    expect(layoutWithTree({}).agents[0].runTree.rowCount).toBe(0)
    expect(layoutWithTree('live').agents[0].runTree).toBeNull()
  })
})

describe('meshLayout makes room for run trees', () => {
  const tallTree = { rowCount: 8 }

  function agentsWith(count, runTree) {
    return createAgents(count).map((agent, index) => (
      index === 0 ? { ...agent, runTree } : agent
    ))
  }

  it('stacks one header per run in the box height', () => {
    const one = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: [{ ...createAgents(1)[0], runTree: { rowCount: 2, runCount: 1 } }],
    }).agents[0].runTree
    const three = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: [{ ...createAgents(1)[0], runTree: { rowCount: 2, runCount: 3 } }],
    }).agents[0].runTree

    expect(three.height - one.height).toBe(2 * RUN_TREE_METRICS.headerHeight)
  })

  it('drops the agent row below the lead tree instead of overlapping it', () => {
    const withoutTree = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(3),
    })
    const withTree = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: { ...createLead(), runTree: tallTree },
      agents: createAgents(3),
    })

    const treeBottom = withTree.lead.runTree.top + withTree.lead.runTree.height
    for (const agent of withTree.agents) {
      expect(agent.y - agent.height / 2).toBeGreaterThanOrEqual(treeBottom)
    }
    expect(withTree.agents[0].y).toBeGreaterThan(withoutTree.agents[0].y)
  })

  it('drops the second row below the first row trees', () => {
    const layout = computeMeshLayout({
      width: 1200,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: agentsWith(8, tallTree),
    })

    const firstRow = layout.agents.filter((agent) => agent.row === 0)
    const secondRow = layout.agents.filter((agent) => agent.row === 1)
    expect(secondRow.length).toBeGreaterThan(0)

    const deepestTree = Math.max(
      ...firstRow
        .filter((agent) => agent.runTree)
        .map((agent) => agent.runTree.top + agent.runTree.height)
    )
    for (const agent of secondRow) {
      expect(agent.y - agent.height / 2).toBeGreaterThanOrEqual(deepestTree)
    }
  })

  it('leaves a tree-free layout exactly where it was', () => {
    const input = {
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(8),
    }
    const layout = computeMeshLayout(input)

    expect(layout.agents.map((agent) => agent.y)).toEqual([
      372, 372, 372, 372, 460, 460, 460, 460,
    ])
  })
})

describe('meshLayout keeps neighbouring run trees apart', () => {
  it('never overlaps the tree boxes of two adjacent nodes', () => {
    const layout = computeMeshLayout({
      width: 960,
      height: 640,
      mode: 'runtime',
      lead: createLead(),
      agents: createAgents(3).map((agent) => ({ ...agent, runTree: { rowCount: 4 } })),
    })

    const boxes = layout.agents
      .map((agent) => agent.runTree)
      .sort((left, right) => left.left - right.left)

    for (let index = 1; index < boxes.length; index += 1) {
      const previous = boxes[index - 1]
      expect(boxes[index].left).toBeGreaterThanOrEqual(previous.left + previous.width)
    }
  })
})
