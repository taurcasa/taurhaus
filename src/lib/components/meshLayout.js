function clamp(value, min, max) {
  if (max < min) return min
  return Math.min(Math.max(value, min), max)
}

function normalizeMode(mode) {
  const value = String(mode || '').toLowerCase()
  if (value === 'runtime' || value === 'initializing') return value
  return 'setup'
}

function normalizeMember(member, fallbackId) {
  return {
    ...member,
    id: String(member?.id ?? fallbackId),
  }
}

function fitHorizontalLayout(rowCount, availableWidth, preferredNodeWidth, preferredGap) {
  if (rowCount <= 0) {
    return { nodeWidth: preferredNodeWidth, gap: preferredGap }
  }

  if (rowCount === 1) {
    return {
      nodeWidth: Math.min(preferredNodeWidth, Math.floor(availableWidth)),
      gap: 0,
    }
  }

  const minGap = 12
  const minNodeWidth = 120
  const hardMinNodeWidth = 48

  let nodeWidth = preferredNodeWidth
  let gap = preferredGap

  const preferredWidth = rowCount * nodeWidth + (rowCount - 1) * gap
  if (preferredWidth <= availableWidth) {
    return { nodeWidth, gap }
  }

  nodeWidth = Math.min(
    preferredNodeWidth,
    Math.max(minNodeWidth, Math.floor((availableWidth - (rowCount - 1) * minGap) / rowCount))
  )
  gap = Math.min(
    preferredGap,
    Math.max(minGap, Math.floor((availableWidth - rowCount * nodeWidth) / (rowCount - 1)))
  )

  if (rowCount * nodeWidth + (rowCount - 1) * gap > availableWidth) {
    gap = minGap
    nodeWidth = Math.max(
      hardMinNodeWidth,
      Math.floor((availableWidth - (rowCount - 1) * minGap) / rowCount)
    )
  }

  return {
    nodeWidth: Math.max(hardMinNodeWidth, nodeWidth),
    gap: Math.max(minGap, gap),
  }
}

/**
 * Geometry of the workflow run tree that hangs under a node.
 *
 * The tree is a sized child box the layout engine places, so the renderer never
 * does arithmetic of its own: it is handed a rectangle and fills it. The row
 * heights live here too, because the box height is `header + rows` and both
 * ends must agree on what a row is worth.
 */
export const RUN_TREE_METRICS = Object.freeze({
  /** Vertical distance from the node's bottom edge to the top of the box. */
  gap: 10,
  paddingX: 8,
  paddingY: 6,
  /** The run name / status line, always rendered. */
  headerHeight: 18,
  /** One phase title or one agent. */
  rowHeight: 16,
  minWidth: 152,
  maxWidth: 240,
  /** Breathing room kept between the box and the canvas edge when it fits. */
  margin: 8,
})

function positiveInteger(value, fallback) {
  const rounded = Math.round(Number(value))
  return Number.isFinite(rounded) ? Math.max(0, rounded) : fallback
}

/**
 * Read a node's run-tree descriptor, or `null` when it carries no run.
 *
 * The descriptor is `{ rowCount, runCount, collapsed }` — how many phase and
 * agent rows the trees want in total, how many run headers they stack, and
 * whether the viewer collapsed them. A tree with no rows is collapsed by
 * definition: there is nothing left to expand.
 */
function runTreeShape(descriptor) {
  if (!descriptor || typeof descriptor !== 'object' || Array.isArray(descriptor)) return null

  const requestedRows = positiveInteger(descriptor.rowCount, 0)
  const collapsed = descriptor.collapsed === true || requestedRows === 0
  const rowCount = collapsed ? 0 : requestedRows
  const runCount = Math.max(1, positiveInteger(descriptor.runCount, 1))

  return {
    rowCount,
    runCount,
    collapsed,
    height: RUN_TREE_METRICS.paddingY * 2
      + runCount * RUN_TREE_METRICS.headerHeight
      + rowCount * RUN_TREE_METRICS.rowHeight,
  }
}

/** Vertical space a node's tree needs below it, including its gap and margin. */
function runTreeClearance(members) {
  let clearance = 0
  for (const member of members) {
    const shape = runTreeShape(member?.runTree)
    if (!shape) continue
    clearance = Math.max(
      clearance,
      RUN_TREE_METRICS.gap + shape.height + RUN_TREE_METRICS.margin
    )
  }
  return clearance
}

/** Place one node's run tree, or `null` when the node carries no run. */
function runTreeBox(node, canvasWidth) {
  const shape = runTreeShape(node?.runTree)
  if (!shape) return null

  const available = Math.max(RUN_TREE_METRICS.minWidth, canvasWidth - RUN_TREE_METRICS.margin * 2)
  const width = clamp(
    Math.max(Number(node.width) || 0, RUN_TREE_METRICS.minWidth),
    RUN_TREE_METRICS.minWidth,
    Math.min(RUN_TREE_METRICS.maxWidth, available)
  )

  return {
    left: clamp(node.x - width / 2, 0, Math.max(0, canvasWidth - width)),
    top: node.y + node.height / 2 + RUN_TREE_METRICS.gap,
    width,
    height: shape.height,
    rowCount: shape.rowCount,
    collapsed: shape.collapsed,
  }
}

function buildRowBoxes(items, y, width, nodeWidth, gap, row) {
  if (!items.length) return []
  const totalWidth = items.length * nodeWidth + (items.length - 1) * gap
  const startX = (width - totalWidth) / 2

  return items.map((agent, column) => ({
    ...agent,
    row,
    column,
    x: startX + column * (nodeWidth + gap) + nodeWidth / 2,
    y,
    width: nodeWidth,
    height: 64,
  }))
}

function routeControlOffset(agent, rowAgents, layout) {
  const laneMidpoint = (rowAgents.length - 1) / 2
  const baseOffset = (agent.column - laneMidpoint) * 8
  const hasMultipleRows = layout.agents.some((candidate) => candidate.row !== agent.row)

  if (Math.abs(baseOffset) >= 8) {
    return baseOffset
  }

  if (hasMultipleRows) {
    const direction = agent.column <= laneMidpoint ? -1 : 1
    const magnitude = agent.row === 0 ? 10 : 24
    return direction * magnitude
  }

  if (rowAgents.length % 2 === 0) {
    return agent.column <= laneMidpoint ? -12 : 12
  }

  if (rowAgents.length >= 3) {
    return 18
  }

  if (layout.agents.length > 1) {
    return agent.column === 0 ? -12 : 12
  }

  return 0
}

function computeMeshTopology(input) {
  const agents = Array.isArray(input?.agents) ? input.agents : []
  const normalizedAgents = agents.map((agent, index) => normalizeMember(agent, `agent-${index}`))
  const count = normalizedAgents.length

  if (count >= 7) {
    const firstRowCount = Math.ceil(count / 2)
    return {
      lead: normalizeMember(input?.lead, 'lead'),
      rows: [
        normalizedAgents.slice(0, firstRowCount),
        normalizedAgents.slice(firstRowCount),
      ],
    }
  }

  return {
    lead: normalizeMember(input?.lead, 'lead'),
    rows: [normalizedAgents],
  }
}

function computeMeshBoxes(topology, input) {
  const width = Math.max(320, Number(input?.width ?? 600))
  const height = Math.max(460, Number(input?.height ?? 460))
  const agents = topology.rows.flat()
  const count = agents.length
  const preferredGap = count >= 7 ? 20 : 28
  const preferredNodeWidth = count >= 7 ? 140 : (count >= 5 ? 160 : 180)
  const maxRowCount = topology.rows.reduce((max, row) => Math.max(max, row.length), 0)
  const availableWidth = Math.max(width, 320)
  const { nodeWidth, gap } = fitHorizontalLayout(
    maxRowCount,
    availableWidth,
    preferredNodeWidth,
    preferredGap
  )

  const lead = {
    ...topology.lead,
    x: width / 2,
    y: Math.round(height * 0.3),
    width: Math.min(width - 24, Math.max(180, nodeWidth)),
    height: 72,
    row: 0,
    column: 0,
  }

  const primaryAgentY = Math.round(height * 0.65)
  const rowOffset = 44
  const agentHeight = 64
  const nodeBreathingRoom = 12
  const firstRow = topology.rows[0] ?? []
  const secondRow = topology.rows[1] ?? []

  // A run tree hangs below its node, so a node that has one pushes whatever sits
  // beneath it further down rather than being drawn over.
  const firstRowY = Math.max(
    secondRow.length > 0 ? primaryAgentY - rowOffset : primaryAgentY,
    lead.y + lead.height / 2 + runTreeClearance([lead]) + agentHeight / 2 + nodeBreathingRoom
  )
  const secondRowY = firstRowY + rowOffset * 2 + runTreeClearance(firstRow)
  const positionedAgents = [
    ...buildRowBoxes(firstRow, firstRowY, width, nodeWidth, gap, 0),
    ...buildRowBoxes(secondRow, secondRowY, width, nodeWidth, gap, 1),
  ]

  const lastAgent = positionedAgents[positionedAgents.length - 1] ?? null
  const addNode = normalizeMode(input?.mode) === 'setup'
    ? (lastAgent
      ? {
        x: lastAgent.x + lastAgent.width / 2 + gap + 24,
        y: lastAgent.y,
      }
      : {
        x: lead.x,
        y: primaryAgentY,
      })
    : null

  return {
    width,
    height,
    lead: { ...lead, runTree: runTreeBox(lead, width) },
    agents: positionedAgents.map((agent) => ({ ...agent, runTree: runTreeBox(agent, width) })),
    addNode,
    nodeWidth,
    gap,
  }
}

function computeMeshRoutes(boxes, input) {
  const agents = Array.isArray(boxes?.agents) ? boxes.agents : []
  if (!agents.length) return []

  const anchorInset = Math.min(28, Math.max(16, Math.round(boxes.lead.width * 0.18)))
  const anchorSpan = Math.max(0, boxes.lead.width - anchorInset * 2)
  const slotById = new Map()
  const hasMultipleRows = agents.some((agent) => agent.row !== agents[0].row)

  if (hasMultipleRows) {
    const uniqueColumns = [...new Set(agents.map((agent) => agent.x))].sort((left, right) => left - right)
    const columnStep = uniqueColumns.length > 1 ? anchorSpan / (uniqueColumns.length - 1) : 0
    const intraColumnOffset = Math.min(18, Math.max(10, columnStep / 2 - 2))
    const groupedAgents = uniqueColumns.map((columnX) =>
      [...agents]
        .filter((agent) => agent.x === columnX)
        .sort((left, right) => left.row - right.row || left.column - right.column)
    )

    groupedAgents.forEach((columnAgents, columnIndex) => {
      const baseSlotX = uniqueColumns.length === 1
        ? boxes.lead.x
        : boxes.lead.x - anchorSpan / 2 + columnStep * columnIndex

      columnAgents.forEach((agent, index) => {
        const offset = (index - (columnAgents.length - 1) / 2) * intraColumnOffset
        slotById.set(agent.id, {
          slotX: clamp(baseSlotX + offset, 0, boxes.width),
          laneIndex: columnIndex * 2 + index,
        })
      })
    })
  } else {
    const sortedAgents = [...agents].sort((left, right) => {
      if (left.x !== right.x) return left.x - right.x
      if (left.row !== right.row) return left.row - right.row
      return left.column - right.column
    })

    for (const [index, agent] of sortedAgents.entries()) {
      const slotX = sortedAgents.length === 1
        ? boxes.lead.x
        : boxes.lead.x - anchorSpan / 2 + (anchorSpan * index) / (sortedAgents.length - 1)
      slotById.set(agent.id, { slotX, laneIndex: index })
    }
  }

  return agents.map((agent) => {
    const rowAgents = agents
      .filter((candidate) => candidate.row === agent.row)
      .sort((left, right) => left.x - right.x || left.column - right.column)
    const lane = slotById.get(agent.id) ?? { slotX: boxes.lead.x, laneIndex: 0 }
    const start = {
      x: lane.slotX,
      y: boxes.lead.y + boxes.lead.height / 2,
    }
    const end = {
      x: agent.x,
      y: agent.y - agent.height / 2,
    }
    const laneY = agent.row === 0
      ? start.y + Math.max(36, (end.y - start.y) * 0.48)
      : start.y + Math.max(56, (end.y - start.y) * 0.64)
    const controlOffset = routeControlOffset(agent, rowAgents, boxes)
    const routeDirection = end.x >= start.x ? 1 : -1
    const control1X = hasMultipleRows && agent.row > 0
      ? start.x
      : clamp(start.x + controlOffset, 0, boxes.width)
    const control2X = hasMultipleRows && agent.row > 0
      ? clamp(end.x + routeDirection * 8, 0, boxes.width)
      : clamp(end.x + controlOffset, 0, boxes.width)

    return {
      id: agent.id,
      fromId: boxes.lead.id,
      toId: agent.id,
      start,
      end,
      control1: {
        x: control1X,
        y: laneY,
      },
      control2: {
        x: control2X,
        y: laneY,
      },
      laneIndex: lane.laneIndex,
      row: agent.row,
    }
  })
}

export function computeMeshLayout(input) {
  const topology = computeMeshTopology(input)
  const boxes = computeMeshBoxes(topology, input)
  return {
    lead: boxes.lead,
    agents: boxes.agents,
    connections: computeMeshRoutes(boxes, input),
    addNode: boxes.addNode,
  }
}
